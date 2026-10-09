# Area 17 — App source layer: edge cases, failure modes, races, tests

There are no `*Test.kt` files in the 27 scope files, so no upstream tests are ported directly. The vectors below are derived from the Kotlin code (traced line by line) and are the acceptance tests for the Rust port. Each vector names the Kotlin lines it derives from.

## 1. Failure modes (what breaks, what the port does)

| # | Failure | Kotlin behaviour (line) | Rust behaviour | Test id |
|---|---|---|---|---|
| F1 | DB error while registering a stub | uncaught in `scope.launch`; plain `Job()` cancels the scope; all later registrations and both collectors die silently (`AndroidMangaSourceManager.kt:40,104-113`) | the single registration logs and returns; other tasks continue | E-F1 |
| F2 | Stub map never filled from DB | `stubSourcesMap` copy is discarded (`:74-82`) | stub table loaded into memory at start | E-F2 |
| F3 | `getOrStub` on a thread that must not block | `runBlocking` DB read on first miss (`:89-93`) | memory only; no DB call | E-F3 |
| F4 | Equality check on stubs always false | `dbSource == source` reference compare (`StubMangaSource.kt` has no equals) → upsert + rename on every emission | `PartialEq` derive; no write when equal | E-F4 |
| F5 | Migration/browse call on a stub id | `sourceManager.get(id) as CatalogueSource` → NPE/TypeCast (`MangaSourceRepositoryImpl.kt:76,81,86`) | `NotCatalogue(id)` error | E-F5 |
| F6 | Torrent check with a vanished extension | `!!` throws NPE (`AnimeSourceExtensions.kt:38`) | returns `false` | E-F6 |
| F7 | Icon load for an uninstalled package | `!!` on package info (`MangaExtensionManager.kt:108`) throws | returns `None`; UI falls back to a generic icon | E-F7 |
| F8 | Metadata DB write fails in a refresh | `updateAnime` returns false, ignored (`UpdateAnimeFromRemote.kt:73`; `AnimeRepositoryImpl.kt:140-148`) | `RemoteError::Db`, sync not run | E-F8 |
| F9 | Cancellation during a refresh | caught as failure (`:83-86`) | `Cancelled` variant, not logged as error | E-F9 |
| F10 | Two language codes with the same localized display name | `TreeMap` keeps one key; a group disappears silently (`GetLanguagesWithMangaSources.kt:29-31`) | tie-break on raw code | E-F10 |
| F11 | `CASE_INSENSITIVE_ORDER` vs `to_lowercase` for a few characters | Java per-char fold | `to_lowercase` (accepted difference, see section 5) | E-F11 |
| F12 | Rename failure during registration | logged; the row is already updated; no retry; orphan folder | same; plus a start-time log of unmatched download folders (Proposal) | E-F12 |
| F13 | Concurrent registrations for one id | no lock; both rename | `register_lock` serialises | E-F13 |
| F14 | Stub id later installed (reinstall) | live wins in `getOrStub`; row rewritten if changed | same | E-F14 |
| F15 | Last-used toggle race (two quick toggles) | non-atomic reads in `ToggleLanguage` and pins | single lock per set | E-F15 |
| F16 | Stored value of enum is unknown | default returned (`PreferenceStore.kt:37-42`) | same | E-F16 |

## 2. Enabled-source list vectors (derived from `GetEnabledMangaSources.kt:31-47` and `GetEnabledAnimeSources.kt:26-38`)

Input rows (id, lang, name), given in repository order:
1. (1, "en", "Zeta")
2. (2, "en", "alpha")
3. (3, "ja", "Beta")
4. (0, "other", "Local manga source")
5. (4, "en", "Hidden")
6. (5, "fr", "Omega")

Settings: languages `{"en", "ja"}`; disabled `{4}`; pinned `{3}`; data-saver excluded `{}`; last used `2`.

Expected output (manga), in order:

| # | id | name | is_used_last | pin | key() |
|---|---|---|---|---|---|
| 1 | 2 | alpha | false | 0b00 | "2" |
| 2 | 2 | alpha | true | 0b00 (Rust); 0b10 (Kotlin XOR) | "2-lastused" |
| 3 | 3 | Beta | false | 0b11 | "3" |
| 4 | 0 | Local manga source | false | 0b00 | "0" |
| 5 | 1 | Zeta | false | 0b00 | "1" |

Derivation: filter removes 5 (fr) and 4 (disabled) and keeps 0 (local via id rule); sort ascending case-insensitive: alpha, Beta, Local, Zeta.

Anime variant: same rows and settings without data saver; output identical except no `data_saver_excluded` field.

Negative vectors:
- E-N1: languages `{}` → output is only row 4 (local).
- E-N2: disabled `{2, 3}` and last used 2 → output is rows 4 and 5 only.
- E-N3: last used `99` (not installed) → no duplicate.
- E-N4: pinned `{3}`, languages `{fr}` → output contains row 5 (Omega, fr) and row 4 (local) only; source 3 absent, and it is still in the pinned set (storage unchanged).
- E-N5: languages `{en, ja, fr}`, last used 5 → Omega appears twice (row 5 and a copy with `is_used_last` true).

## 3. Migration list vectors (derived from `GetMangaSourcesWithFavoriteCount.kt:30-57`)

Input rows, repository order: a "Zeta" live 3; b "alpha" live 10; c "Beta" live 3; d "Gone" stub 7; e "Mid" live 10.

| Test | Mode | Direction | Expected ids |
|---|---|---|---|
| E-M1 | ALPHABETICAL | ASCENDING | d, b, c, e, a |
| E-M2 | ALPHABETICAL | DESCENDING | a, e, c, b, d |
| E-M3 | TOTAL | ASCENDING | d, a, c, b, e |
| E-M4 | TOTAL | DESCENDING | b, e, a, c, d |
| E-M5 | ALPHABETICAL | ASCENDING, plus a local row (id 0) in input | local row absent from the output |
| E-M6 | ALPHABETICAL | ASCENDING, two live rows named "same" and "SAME" (collator primary equal) | input order kept between them |

Rust asserts E-M4 with `Vec` equality, not as a set.

## 4. Remote update vectors (derived from `UpdateAnimeFromRemote.kt:142-214` and `:161-191`)

Notation: `local(fav, thumb, bg, isLocal, hasCustomCover, hasCustomBg)`, `remote(thumb, bg, title, author, genre, status)`, `manual`.

Cover rule, expected `coverLastModified`:

| Test | Local | Remote | manual | Expected cover | Cache action |
|---|---|---|---|---|---|
| E-R1 | thumb "a" | thumb "" | false | keep (null) | none |
| E-R2 | thumb "b" | thumb "b" | false | keep (null) | none |
| E-R3 | thumb "b" | thumb "b" | true | now | delete (not custom) |
| E-R4 | id 0 (local), thumb "a" | thumb "b" | false | now | none |
| E-R5 | thumb "a", custom cover exists | thumb "b" | false | keep (null) | delete |
| E-R6 | thumb "a", no custom | thumb "b" | false | now | delete |
| E-R7 | thumb null | thumb "b" | false | now | none (row 5 runs, but `getCoverFile(null)` returns null, so there is no file to delete) |

Implementation must not panic on a null local URL.

Background rule: identical table with `bg` and `hasCustomBg`.

Title rule:

| Test | Local favourite | Remote title | Expected title |
|---|---|---|---|
| E-T1 | true | "X" | null |
| E-T2 | false | "X" | "X" |
| E-T3 | false | "" | null |
| E-T4 | false | not initialised (exception) | null |

Field rules:

| Test | Remote field | Expected stored |
|---|---|---|
| E-U1 | genre `", ,A, A,B"` | `["A","B"]` |
| E-U2 | genre `null` | unchanged (null passed) |
| E-U3 | genre `"   "` (blank) | unchanged (getGenres returns null) |
| E-U4 | status `0` | status written `0` |
| E-U5 | author `""` | author written as `""` (only null keeps) |
| E-U6 | author `null` | unchanged |
| E-U7 | memo `{}` | memo written `{}` |
| E-U8 | thumbnail_url `""` | thumbnailUrl null (unchanged) |
| E-U9 | update_strategy `ONLY_FETCH_ONCE` | written |
| E-U10 | initialized | always `true` |

Episode and season flow:

| Test | Setup | Expected |
|---|---|---|
| E-S1 | source returns 0 episodes, source not local | `Result.failure(NoEpisodesException)`; metadata already written (Kotlin order) |
| E-S2 | source returns 0 episodes, local source | success with `new_episodes = []` |
| E-S3 | stub source | failure `NotInstalled`; no DB write |
| E-S4 | metadata write returns false | Rust: failure `Db`, no episode sync; Kotlin: sync runs (documented difference) |
| E-S5 | cancelled after the source call | Rust: `Cancelled`; no failure log; Kotlin: `Result.failure` |
| E-S6 | seasons: source returns 0 seasons, not local | failure `NoSeasonsException` (`SyncSeasonsWithSource.kt:31-32`) |
| E-S7 | episodes list ordering | fed to the source sorted ascending by source order |

## 5. Accepted differences from Kotlin (documented, tested)

1. `to_lowercase` vs Java `CASE_INSENSITIVE_ORDER` for a handful of code points (e.g. German `ß`, Turkish dotted I). Test E-F11 pins the Rust output for the names `['ß', 'ss', 'st']` and it is reviewed by hand against the Java collator order.
2. Stub listing shows all stored stubs (Kotlin shows only those touched this session).
3. Pin bit on unpinned last-used row is `0b00` (Kotlin `0b10`). No consumer reads the bit (E-N2 covers the visible output).
4. `Cancelled` is never a failure.
5. Metadata write failure stops the episode sync.

## 6. Concurrency tests

| Test | Steps | Expected |
|---|---|---|
| E-C1 | run 50 snapshot emissions with the same extension list | exactly one row write per source (after PartialEq), no rename calls |
| E-C2 | two registrations for id 10 with different names concurrently | final row holds the later write; rename called once per distinct change; no panic |
| E-C3 | flip disabled for id 4 twice from two threads | final membership is deterministic: id 4 is in the set iff the number of completed flips is odd (lock held, no lost update) |
| E-C4 | refresh the same anime twice concurrently (Proposal per-anime mutex) | both complete; episodes not duplicated |
| E-C5 | `get_or_stub` called from a thread while the stub table reloads | returns either the old or new value, never panics |
| E-C6 | UI thread call of `get_or_stub` with 10k unknown ids | completes under 5 ms per call on average (no DB) |

## 7. Start-up and readiness tests

| Test | Steps | Expected |
|---|---|---|
| E-B1 | start with no extensions | `ready` becomes true after the first snapshot; local source present; stub list from DB |
| E-B2 | start with the DB unavailable | registry starts with an empty stub map; errors logged; `ready` still true after the snapshot |
| E-B3 | `ready_watch` subscribed before start | receives `false` then `true` exactly once |

## 8. Fixtures needed

- `fixtures/sources/manga_rows.json`: the 6 rows of section 2 and the 5 rows of section 3 as `SourceEntry` values.
- `fixtures/sources/stub_rows.sql`: `INSERT INTO sources VALUES (10,'en','Foo'),(20,'','Name'),(21,'en','   '),(999,'ja','Bar')` for stub tests S7 and S8.
- `fixtures/remote/sanime_*.json`: serialized `SAnime`-shaped values for E-U1..E-U10 (fields: `title`, `thumbnail_url`, `background_url`, `artist`, `author`, `status`, `description`, `genre`, `update_strategy`, `memo`).
- An in-memory `StubStore` implementation (`HashMap` + a `watch` counter) and a fake `ExtensionCatalog` that publishes snapshots on demand.
- A fake `SourceRenamer` that records `(old, new)` calls.
- A fake `AnimeSourceLike` whose methods return canned `SAnime`/episode lists or fail with `NotInstalled`.

## 9. Coverage target

- Pure modules (`pin`, `naming`, `enabled`, `languages`, `migration`, `incognito`, `datasaver`, `torrent`): 100% of branches; each function has at least one vector from sections 2 to 4.
- The `if` ladder in `cover_last_modified` and `background_last_modified`: one test per row of the decision table in `p1-topic-remote-entry-update.md` 5.2 and 5.3.
- Registry and remote update: the vectors in sections 6 and 7.

## 10. Manual checks (for the solo developer)

1. Install a source, close the app, uninstall the extension, reopen: the source still appears in the stub list and in library entries with its stored name.
2. Rename an extension's source (test build), reopen: the download folder follows (check `ls` of the download directory).
3. Pin a source, untick its language, reopen the Browse tab: the source is hidden; tick the language again: it is pinned again.
4. Migration list: toggle ALPHABETICAL and TOTAL, ASC and DESC; compare with section 3.
5. Refresh one anime with the network off: the error message appears once, the library entry is unchanged.
