# Area 06 — Edge cases, failure modes, races, tests

Paths repo-relative to `/home/blackbox/aniyomi-src`. "T-06a-nnn" refers to `p1-tasks.md`. Test code is Rust in `kuro-core` (`cargo test -p kuro-core`) unless noted.

## 1. Edge cases and failure modes

### 1.1 Database handle and notifications

| # | Situation | Required behaviour | Test |
|---|---|---|---|
| D1 | Closure returns `Err` inside `write` | Transaction rolled back, no notification published, error returned | `write_err_rolls_back` |
| D2 | Closure panics inside `write` | Transaction rolled back (guard drop), writer mutex still usable (parking_lot has no poisoning), panic resumes in the awaiting task as `JoinError` mapped to `DbError::Join` | `write_panic_does_not_poison` |
| D3 | `write_blocking` called inside a `write` closure | `Err(DbError::NestedWrite)` (and `debug_assert` failure in debug) instead of deadlock | `nested_write_is_rejected` |
| D4 | Awaiting future dropped mid-`write` | The closure still commits (documented non-cancellation); no partial state | `dropped_future_still_commits` |
| D5 | 100 inserts in one `write`, one watcher | Watcher emits exactly 2 values (initial + one after commit) | `burst_is_coalesced` |
| D6 | 100 separate `write`s in a tight loop, one watcher | At least 2 and at most 101 emissions, the last value reflects all 100 rows (latest-wins) | `watch_converges_to_latest` |
| D7 | `DELETE FROM history WHERE 1` (delete all) | `History` notification fires (truncate optimisation avoided) | `delete_all_notifies` |
| D8 | Chapter deleted with `ON DELETE CASCADE` history row | `History` and `Chapters` both notify | `cascade_notifies_child_table` |
| D9 | Constraint violation (`UNIQUE` fingerprint) inside `write` | `DbError::Sqlite` with `is_constraint() == true`; rolled back | `constraint_error_flag` |
| D10 | Opening a file whose `user_version` is newer than the schema | `Db::open` returns `DbError::Migration("database is newer than this app")` and does not modify the file (area:03/04 own the message) | `open_newer_version_fails` |
| D11 | Another process holds the write lock for > 5 s | `write` returns `DbError::Sqlite(DatabaseBusy)`; UI shows `internal_error`; no retry loop | manual |
| D12 | `PRAGMA foreign_keys` on writer and on a pooled reader | both 1 | `pragmas_on_every_connection` |
| D13 | Subscription dropped while a re-query is running | Task finishes the query, notices closed channel, exits; no panic, no leak (checked with `Arc::strong_count` of the closure's captured Arc) | `subscription_drop_stops_task` |
| D14 | Broadcast lag (writer outpaces watcher) | `RecvError::Lagged` treated as "all tables changed": one re-run | `lagged_triggers_rerun` |

### 1.2 History

| # | Situation | Required behaviour |
|---|---|---|
| H1 | Upsert for a chapter id that does not exist (FK violation) | Error is logged, returns normally, no row (Kotlin: caught `Exception`) |
| H2 | Two upserts in the same millisecond for two chapters of one manga | List shows exactly one row for that manga (one of the tied chapters); never zero, never two |
| H3 | Reset the only history row of a manga | Manga leaves the list; `time_read` kept; `total_read_duration` unchanged |
| H4 | `delete_all` then `total_read_duration` | 0 (coalesce) |
| H5 | `upsert` with `session_read_duration` 0 | `last_read` updated, `time_read` unchanged |
| H6 | Negative `session_read_duration` (clock moved backwards) | `HistoryRecorder` clamps to 0 before calling; the repository stores what it is given (parity) |
| H7 | Incognito toggled on between `start()` and `flush()` | No write (flag read at flush time, as Kotlin reads `incognitoMode` inside `updateHistory`) |
| H8 | Search text contains `%`, `_`, `\` | Literal match with the improvement on (`ESCAPE '\'`); wildcard match in strict-parity mode (flag `HISTORY_SEARCH_PARITY`, default off) |
| H9 | Search "éclair" against title "Éclair" | Found with `kuro_fold`; not found in parity mode |
| H10 | Title is an empty string | Matched by the empty query only (and by every query that is a substring of "" i.e. only empty) |
| H11 | History row whose chapter belongs to a manga with `favorite = 0` | Still listed, `cover.is_manga_favorite = false` |
| H12 | Anime upsert for an episode with a later `seen_at` older than the stored one | `last_seen` is overwritten with the given value (no max) — parity |

### 1.3 Updates

| # | Situation | Required behaviour |
|---|---|---|
| U1 | Chapter with `date_upload = 0` | Never listed |
| U2 | `date_fetch == date_added` | Not listed (strict `>`) |
| U3 | Un-favourite then favourite again | Older chapters (`date_fetch <= new date_added`) disappear |
| U4 | 600 qualifying chapters | 500 newest by `date_fetch` listed |
| U5 | Chapter `read` toggled | Watcher re-emits with the new `read` |
| U6 | `after` exactly equals `date_upload` | Not listed (`>`) |
| U7 | Three-month window computed on 31 May | The day-of-month is clamped like `ZonedDateTime.minusMonths(3)`: 31 May 2026 14:00 local -> 28 Feb 2026 14:00 local; 31 May 2028 14:00 local -> 29 Feb 2028 14:00 local (leap year) |

### 1.4 Tracks

| # | Situation | Required behaviour |
|---|---|---|
| T1 | `insert` for (manga 1, tracker 2) twice | One row, id differs between the two inserts (REPLACE); `by_id(old_id)` -> `None` |
| T2 | `insert_all` where the 3rd element violates NOT NULL | Whole batch rolled back (single transaction) |
| T3 | Delete the manga | Track rows cascade away; the all-tracks watcher notifies |
| T4 | `by_id` with a manga id by mistake | Returns an unrelated row or `None` (documented footgun; the Rust name `by_id` and doc comment make it explicit) |
| T5 | Coalesced update with all `None` | Row unchanged |
| T6 | `copy_personal_from(other, false)` | `private` unchanged; other five fields copied |
| T7 | `last_chapter_read` fractional (12.5) | Round-trips exactly (REAL) |

### 1.5 Sources

| # | Situation | Required behaviour |
|---|---|---|
| S1 | Source installed -> removed while "with favourite count" is watched | Re-emits; that source's row flips `is_stub = true` and keeps its stored name/lang |
| S2 | Stub table has no row and the extension list has no data | `Source { name: "", lang: "", is_stub: true }`; `visual_name` = name when lang empty -> `""`; the UI falls back to the numeric id (matches `StubMangaSource.toString`) |
| S3 | Pager: page 1 non-empty with `has_next`, page 2 empty | `append()` returns `Err(NoResults)`; `has_next()` stays true; a second `append()` retries page 2 |
| S4 | Pager: source throws on page 3 | Error returned, `next_key` still 3, items of pages 1-2 untouched |
| S5 | `refresh()` after a failed append | Starts from page 1 again |
| S6 | Manga `pager(id)` for an uninstalled source | `Err(SourceNotInstalled)` (Kotlin: NPE) |
| S7 | Search with `query` equal to the old sentinel string | Treated as a normal search (sentinels not ported) |
| S8 | Source reports `has_next_page = true` forever with identical pages | Pager does not detect loops (parity); UI de-dup by local row hides repeats |
| S9 | Two stubs with the same id upserted concurrently | Last writer wins; no error (`ON CONFLICT DO UPDATE`) |

### 1.6 Extension stores and repos

| # | Situation | Required behaviour |
|---|---|---|
| E1 | Empty body | `StoreError::EmptyBody`, nothing written |
| E2 | Body is HTML (a Cloudflare page, starts with `<`) | Falls to the protobuf branch: decode error `StoreError::Proto`; message shown in the dialog; nothing written. Proposal improvement: if the first non-space byte is `<`, return `StoreError::NotAnIndex("received an HTML page")` |
| E3 | `[`-array URL not ending in `/index.min.json` | `StoreError::LegacyUrlInvalid` ("Provided legacy store url is not valid") |
| E4 | Legacy URL containing `/index.min.json` twice | Both replaced (Kotlin `String.replace`) |
| E5 | `repo.json` with `index_v2` pointing at itself | `TooManyIndexHops` after 4 requests (Kotlin loops forever) |
| E6 | `repo.json` where `index_v2` is an empty string | Followed as a URL: request error (Kotlin same). Proposal: treat empty as absent |
| E7 | gzip payload whose inflated size exceeds 32 MiB | `StoreError::TooLarge`, nothing written |
| E8 | gzip magic but corrupt stream | `StoreError::Gzip` |
| E9 | JSON with BOM / leading whitespace | Accepted (improvement); Kotlin sends it down the protobuf path and fails |
| E10 | New-format store with `extensionListUrl` AND inline `extensionList` | `extensionListUrl` wins in `get_extensions` (first branch) |
| E11 | Store without `extensionList` and without `extensionListUrl`, `is_legacy = false` | `get_extensions` fails (`MissingExtensionList`); `fetch_extensions` logs and skips the store |
| E12 | Extension with `extensionLib = "abc"` | The store's whole list fails (`Invalid("extensionLib")`); other stores unaffected |
| E13 | Zero sources in an extension | `lang = "all"`, `sources = []` |
| E14 | Sources in two languages | `lang = "all"`; per-source languages preserved |
| E15 | `contentWarning = MIXED` | `is_nsfw = true`; `SAFE`/`UNSPECIFIED` false |
| E16 | Legacy index entry without `sources` | One synthetic source `(0, name, lang, "")` |
| E17 | Legacy `name = "Aniyomi: Foo"` | `"Foo"`; `"Foo"` stays `"Foo"` |
| E18 | Legacy `version = "1.5.12"` | `lib_version = 1.5`; `"1"` (no dot) -> `substringBeforeLast('.')` returns the whole string -> 1.0 |
| E19 | Adding a store that already exists (same `index_url`) from code (not UI) | Upsert overwrites; no error |
| E20 | Refresh of a store whose host is down | Old row preserved; ERROR log; other stores still refreshed |
| E21 | Remove a store while `fetch_extensions` runs | `fetch_extensions` works on its snapshot; result may include the removed store's extensions once |
| E22 | Manga repo: URL `http://x/index.min.json` | `InvalidUrl` |
| E23 | Manga repo: URL `https://x/index.min.json?x=1` | Normalised URL ends with `?x=1`, fails the suffix check -> `InvalidUrl` |
| E24 | Manga repo: fingerprint equals an existing repo with a different base URL | `DuplicateFingerprint(old, new)`; `replace(new)` rewrites the old row's `base_url` |
| E25 | `replace` where `new.base_url` already belongs to a third repo | `Err` (PK conflict) — surfaced as a log + `internal_error` toast instead of Kotlin's crash |
| E26 | `update_all` where one repo changed its fingerprint | That repo untouched; others updated |
| E27 | Migration: legacy URL list `["https://a/", "https://b/"]` | rows `("https://a/", "Repo #1", NULL, "https://a/", "NOFINGERPRINT-1")` and `("https://b/", "Repo #2", NULL, "https://b/", "NOFINGERPRINT-2")` |
| E28 | Anime migration URL `https://h/index.min.json` and `https://h/index.json` and `https://h/x` | inserted `index_url`: `https://h/repo.json`, `https://h/repo.json`, `https://h/x/repo.json` (`removeSuffix` twice then append) |
| E29 | Deep link `aniyomi://add-repo?url=https%3A%2F%2Fh%2Frepo.json` | Confirm dialog shows `https://h/repo.json`, `already_exists` computed by exact match |

### 1.7 Custom buttons

| # | Situation | Required behaviour |
|---|---|---|
| C1 | Fresh database | Exactly one button: id 1, `+85 s`, favourite, sort 0, seed texts byte-identical |
| C2 | Delete the middle of three buttons | Remaining have `sortIndex` 0, 1 |
| C3 | Reorder with `new_index = len` | Error -> `internal_error` toast, order unchanged |
| C4 | Toggle favourite on a non-favourite while another is favourite | Exactly one favourite afterwards (the target) |
| C5 | Toggle favourite on the favourite | Zero favourites |
| C6 | Two buttons with the same name | Allowed |
| C7 | `expand_placeholders("$identity $isPrimary", 7, 7)` | `"7entity true"` |
| C8 | `expand_placeholders("x$idy", 12, 99)` | `"x12y"` |
| C9 | Delete the favourite | No favourite remains; player shows no custom primary button |
| C10 | Script with a Lua syntax error | Only that button's file fails to load (per-button files) |
| C11 | Zero buttons | `build_player_scripts` returns no files and the old files are deleted |

### 1.8 Release

| # | Situation | Required behaviour |
|---|---|---|
| R1 | Release has no matching asset | `latest` returns `None` -> "no new update" |
| R2 | Two assets with the same arch tag | The later one wins |
| R3 | `body` null/missing | Decodes (improvement), `info = ""` |
| R4 | HTTP 403 rate-limited | `ReleaseError::Http(403)`; manual check shows the message; startup check logs only |
| R5 | Tag `v0.18.3`, installed `0.18.2.1` | New version (padded compare) — Kotlin throws `IndexOutOfBounds` |
| R6 | Tag `v0.17.9`, installed `0.18.2` | Not new (Kotlin says new) |

## 2. Race conditions to test or reason about

1. Reader pool read racing a write: a watcher triggered by commit N must never observe state older than N (publish after commit; test with a writer thread incrementing a counter and an assertion that every emitted value >= the number of notifications received).
2. Two `toggle_favorite` calls interleaved: both run inside serialised writer transactions, final state has exactly one favourite (test with 50 concurrent tasks).
3. `create` of custom buttons from two tasks: sort indexes distinct and consecutive (atomic `max+1` inside the write closure).
4. `refresh_all` and `add` overlapping on the same URL: both use upsert; last write wins; table has one row.
5. History `flush` during app shutdown: `spawn_blocking` closure completes if the runtime is still alive; `AppContext::shutdown` (area:01) must `await` outstanding history writes (`JoinSet`) before dropping the runtime — otherwise the last session's duration is lost.
6. `delete_all` history concurrent with `upsert`: serialised; the upsert after the delete creates a fresh row.

## 3. Fixtures needed

`kuro_core::db::testkit` (built on the schema from areas 03/04; builder methods default every NOT NULL column so tests state only what matters):

```rust
let (db, _dir) = Db::open_temp(DbKind::Manga, &MANGA_SCHEMA);
let f = Fixture::manga(&db);
let m = f.manga("One Piece").favorite(true).date_added(1_700_000_000_000).source(1).insert();    // -> MangaId
let c1 = f.chapter(m).number(10.0).date_fetch(1_700_000_100_000).date_upload(1_700_000_050_000).insert();
f.history(c1).last_read(300).time_read(5000).insert();
```

Anime counterpart `Fixture::anime`. A `FakeSource` (scripted pages), `FakeSourceManager` (watch channel + stubs map), `FakeHttp` (returns canned bodies by URL, area:12 testkit) complete the set. Hand-built store documents (all under `kuro-core/tests/fixtures/stores/`):

| File | Content |
|---|---|
| `modern.json` | New-format store with 2 extensions (one single-language, one multi-language, one `MIXED`, one `isTorrent`) |
| `modern.pb` | The same store encoded with `prost` (generated by a test helper, committed after a golden check) |
| `modern.json.gz` | gzip of `modern.json` |
| `modern_list_url.json` | store with `extensionListUrl` + `list.json` (`{"extensions": [<the two extensions of modern.json>]}`) and `list.pb` |
| `legacy_repo.json` | `{"meta": {"name": "Aniyomi Extensions", "shortName": "Ani", "website": "https://example.org", "signingKeyFingerprint": "AB12"}}` |
| `legacy_repo_v2.json` | the same with `"index_v2": "https://example.org/v2/index.json"` |
| `legacy_index.json` | array of 3 entries (named `Aniyomi: A`, with sources / without sources / nsfw 1) |
| `html.txt` | `<html><body>Just a moment</body></html>` (Cloudflare interstitial) |

All hosts in fixtures use `example.org`/`example.test`; no network.

## 4. Test list per subsystem

Naming: `kuro-core/src/<module>/tests.rs` or `kuro-core/tests/<area>_*.rs`.

* `db`: D1-D14 above.
* `history`: H1-H12; plus `view_latest_per_manga_table` (section 1 of the history/updates topic, five rows); `upsert_accumulates` (60000 + 30000 ms -> 90000; reset keeps; later 5000 -> 95000); `by_manga_id_includes_reset_rows`; `reset_by_manga_id_only_that_manga`; `latest_matches_first_list_row`; `anime_upsert_has_no_duration_column`.
* `updates`: U1-U7; `limit_500`; `mapper_drops_date_upload`; `ordering_stable_with_equal_fetch`.
* `track`: T1-T7; `group_by_manga`; `anime_track_fields`.
* `source`: S1-S9; `with_favorite_count_combines`: emits nothing until both inputs emitted; `online_sources_exclude_local`; `non_library_counts`.
* `extension::store`: E1-E21, `wire_json_roundtrip`, `wire_proto_roundtrip` (field numbers 1-4, 101, 102; enum values 0-3), `content_warning_alt_names`, `unknown_enum_name_fails`, `mapping_table` (rows of the mapping table in `p1-data-and-config.md`), `refresh_all_continues_after_failure`, `fetch_extensions_preserves_store_order`, `upsert_overwrites_signing_key_and_logs_warn`.
* `extension::repo`: E22-E29; `create_flow_success`, `create_flow_exists`, `create_flow_duplicate_fingerprint`, `update_adopts_fingerprint_from_NOFINGERPRINT`, `update_rejects_changed_fingerprint`.
* `custom_button`: C1-C11; `seed_matches_schema_sql` (compares constants to the SQL file text from area:04).
* `release`: R1-R6, mention vectors (section 5), `pick_download_link` cases.

## 5. Upstream tests (`domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt`) and release vectors

This file is in area:09's scope; its four cases validate the interactor that consumes `ReleaseServiceImpl`'s output, so they are mirrored here as integration tests of the Rust `release` module + area:09's `is_new_version` (identical inputs):

| Case (Kotlin test name) | Arguments | Service returns | Expected |
|---|---|---|---|
| `When has update but is preview expect new update` | `isPreview = true, commitCount = 1000, versionName = ""`, last check 0 | `Release("r2000", "info", "http://example.com/release_link", "http://example.com/release_link.apk")` | `NewUpdate(release)` |
| `When has update expect new update` | `isPreview = false, commitCount = 0, versionName = "v1.0.0"`, last check 0 | `Release("v2.0.0", "info", "http://example.com/release_link", "http://example.com/release_link.apk")` | `NewUpdate(release)` |
| `When has no update expect no new update` | `isPreview = false, versionName = "v2.0.0"`, last check 0 | `Release("v1.0.0", "info", "http://example.com/release_link", "http://example.com/release_link.apk")` | `NoNewUpdate` |
| `When now is before three days expect no new update` | `versionName = "v2.0.0"`, last check = now | not called (`coVerify(exactly = 0)`) | `NoNewUpdate` |

Mention linkifier vectors (`linkify_mentions`):

| Input | Output |
|---|---|
| `thanks @octocat!` | `thanks [@octocat](https://github.com/octocat)!` |
| `foo@bar` | unchanged |
| `(@bob)` | `([@bob](https://github.com/bob))` |
| `@Bob` | `[@Bob](https://github.com/Bob)` |
| `@a-b` | `[@a-b](https://github.com/a-b)` |
| `@-ab` | unchanged |
| `@ab-` | `[@ab](https://github.com/ab)-` |
| `@a--b` | `[@a](https://github.com/a)--b` |
| `@user_name` | `[@user](https://github.com/user)_name` |
| `@` followed by 45 `a` | link text `@` + exactly 39 `a`, link target `https://github.com/` + exactly 39 `a`, then the remaining 6 `a` as plain text |
| `@@bob` | `@[@bob](https://github.com/bob)` |
| `email@x.com` | unchanged |
| `@bob and @alice` | both linked |

Asset selection vectors (`pick_download_link`, arch `x86_64`): `[k-1.0-x86_64.tar.zst, k-1.0-aarch64.tar.zst]` -> first; `[k-1.0.tar.zst]` -> that (key None); `[k-1.0-aarch64.tar.zst]` -> `None`; `[k-1.0.tar.zst, k-1.0-x86_64.tar.zst]` -> the tagged one; duplicates -> last.

## 6. Manual verification checklist (after wiring the mockup, T-06a-080/081)

1. Read one chapter for 10 s in the reader, leave: History shows the manga with the chapter, time `HH:mm`; stats read-time grows by ~10 s.
2. Remove via trash with the switch off: the previous chapter of the manga appears (if any); with the switch on: the manga disappears.
3. Clear history: snackbar "History deleted", list empty, stats read-time 0.
4. Add an extension store from a local web server serving `modern.json`; refresh; stop the server; refresh again (error logged, row stays).
5. Custom button: delete the seeded button, restart, it does not come back; add two, reorder, favourite one.
6. Browse a source: scroll to the end; error row on a throttled page; Retry loads the next page.
