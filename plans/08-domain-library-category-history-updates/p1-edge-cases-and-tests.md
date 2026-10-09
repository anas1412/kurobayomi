# Area 08 — Domain: library, category, history, updates — edge cases, failure modes, tests

## 1. Upstream tests to port (all of them)

The scope has exactly one upstream test file: `domain/src/test/java/tachiyomi/domain/library/model/LibraryFlagsTest.kt` (JUnit5 + kotest, class annotated `@Execution(ExecutionMode.CONCURRENT)`). Port to `crates/kuro-domain/tests/library_flags.rs`:

| Upstream test | Assertions (exact) | Rust test |
|---|---|---|
| `Check the amount of flags` | `LibraryDisplayMode.values.size == 4`; `MangaLibrarySort.types.size == 10`; `MangaLibrarySort.directions.size == 2`; `AnimeLibrarySort.types.size == 11`; `AnimeLibrarySort.directions.size == 2` | `assert_eq!(LibraryDisplayMode::ALL.len(), 4); assert_eq!(SortType::MANGA.len(), 10); assert_eq!(SortType::ANIME.len(), 11); assert_eq!(Direction::ALL.len(), 2)` |
| `Test Flag plus operator (LibrarySort)` | `MangaLibrarySort(LastRead, Ascending) + MangaLibrarySort(DateAdded, Ascending) == 0b01011100`; same for anime (`LastSeen`, `DateAdded`) | `LibrarySort{LastActivity,Asc}.merge_with(LibrarySort{DateAdded,Asc}) == 0b0101_1100` for both kinds |
| `Test Flag plus operator` | `MangaLibrarySort(DateAdded, Ascending).flag == 0b01011100`; anime same | `flag()` assertion |
| `Test Flag plus operator with old flag as base` | `MangaLibrarySort(UnreadCount, Descending).flag == 0b00001100` (anime `UnseenCount`); `currentSort.flag + MangaLibrarySort(DateAdded, Ascending) == 0b01011100` and `!= 0b00001100`. **Note** the anime half of the upstream test computes `animesort.flag + animesort` (uses the new sort as base, an upstream typo): it asserts `0b01011100`, true for any base; the Rust test uses the intended base (`UnseenCount|Desc`) and asserts the same constants | `merge(0x0C, 0x5C, 0x7C) == 0x5C` |
| `Test default flags` | `default.type + default.direction == 0b01000000` for both kinds | `LibrarySort::DEFAULT.flag() == 0x40` |

## 2. New unit tests (vectors included so the test can be written without the Kotlin)

### 2.1 Flags and sort codecs (T-08a-002, T-08a-003)

- `Direction::from_flags(f)`: `0 -> Descending`, `0x40 -> Ascending`, `0xFF -> Ascending`, `0x3C -> Descending`.
- `SortType::from_flags(Manga, f)`: `0x00 Alphabetical, 0x04 LastActivity, 0x08 LastUpdate, 0x0C UnreadCount, 0x10 TotalItems, 0x14 LatestItem, 0x18 ItemFetchDate, 0x1C DateAdded, 0x20 TrackerMean, 0x3C Random`; unassigned `0x24, 0x28, 0x2C, 0x30, 0x34, 0x38` -> Alphabetical (manga) and anime maps `0x30 -> AiringTime` while `0x24, 0x28, 0x2C, 0x34, 0x38 -> Alphabetical`; `from_flags(kind, None) == DEFAULT`.
- Round trip: for every (kind, valid type, direction): `LibrarySort::from_flags(kind, Some(s.flag())) == s`.
- Bits preserved: `apply_to(0x01 | 0x80, DateAdded Asc) == 0x81 | 0x5C` (bits outside `0x7C` stay).
- String codec: `"ALPHABETICAL,ASCENDING"` <-> default; `""` -> default; `"LAST_READ"` (no comma) -> default (not LastRead); `"LAST_READ,SIDEWAYS"` -> (LastActivity, Descending); `"WAT,ASCENDING"` -> (Alphabetical, Ascending); `"LAST_SEEN,ASCENDING"` parsed as Manga -> (Alphabetical, Ascending); `"LAST_SEEN,ASCENDING"` as Anime -> (LastActivity, Ascending); `"RANDOM,DESCENDING"` round-trips; `serialize` never emits a single token.
- `LibraryDisplayMode`: `"COMPACT_GRID", "COMFORTABLE_GRID", "COVER_ONLY_GRID", "LIST"` round trip; `"grid"`, `""`, `"compact_grid"` -> `CompactGrid`.

### 2.2 Library pipeline (T-08a-013 to T-08a-016)

Fixture library (manga, kind = Manga), titles and fields:

| id | title | cats | total/read | lastRead | latestUpload | fetched | dateAdded | status |
|---|---|---|---|---|---|---|---|---|
| 1 | "Alpha" | 0 | 10/10 | 500 | 100 | 10 | 1 | 2 (completed) |
| 2 | "beta" | 0, 5 | 8/2 | 900 | 300 | 20 | 2 | 1 |
| 3 | "Gamma" | 5 | 5/0 | 0 | 200 | 30 | 3 | 1 |
| 4 | "Émile" | none | 3/3 | 100 | 50 | 5 | 4 | 0 |

- Display categories: categories visible = [Default(0), Hold(5)]. Rows: id1->0, id2->0, id2->5, id3->5, id4->0 (no category = 0). Default present -> both pages. Remove ids 1, 2, 4 from Default (move them to Hold): rows only have category 5 -> library non-empty without category-0 rows -> Default page hidden, pages = [Hold]. Empty library -> pages = [Default, Hold] each empty.
- Sorting (ascending flags) Alphabetical within Default: `Alpha, beta, Émile` (primary-strength collation: `É` sorts with `E`, case ignored, `beta` after `Alpha`, `Émile` after `beta`); Descending: `Émile, beta, Alpha`.
- UnreadCount (unread = total - read): ids 1: 0, 2: 6, 3: 5, 4: 0. Ascending page Default {1,2,4}: expected `2, Alpha, Émile` (zeros last; tie-break alphabetical among zeros: `Alpha`, `Émile`). Descending: comparator reversed but zeros still last: `2, Alpha, Émile`. Hold page {2,3}: ascending `3 (5), 2 (6)`; descending `2, 3`.
- LastRead ascending Default: ids by lastRead `4 (100), 1 (500), 2 (900)`; descending `2, 1, 4`; tie on equal values broken A-Z.
- TrackerMean: entries without scores use -1.0; scores `2 -> 8.5`, `3 -> 9.0`, `1 -> NaN-case` (all tracks unscored): treat as -1.0.
- Random with seed 42: two calls produce the identical order; after the seed changes the order differs (assert on a 20-item list that the permutation is not identical with probability 1 - 1/20!: use two fixed seeds, assert both permutations are valid permutations and differ).
- Filters: downloaded `ENABLED_IS` keeps local entries (`source == 0`) and `download_count > 0`; `ENABLED_NOT` negates; unread `ENABLED_IS` -> ids 2, 3; started `ENABLED_NOT` -> id 3; completed `ENABLED_IS` -> id 1; bookmarked via `bookmarks > 0`; interval-custom ignored when `skip_outside_release_period` is false even when set; tracking: trackers {1: IS, 2: NOT}, entry tracked on [1,2] -> excluded; on [1] -> included; on [] -> excluded (included non-empty and none match); with both maps empty/DISABLED -> unaffected; with no logged-in trackers -> unaffected even if stale prefs say IS.
- Search: `"alp"` matches id1 by title; `"ALP"` too; `"id:2"` matches only id 2; `"id:abc"` matches none; `"id:"` matches none (`toLongOrNull("") == null`); description match; `"action, -isekai"` with genres [Action, Drama] -> matches (genre `action` equals sub1 and `-isekai` not a genre/source substring); source name `contains`; `"-drama"` -> entry with genre Drama does not match via genre branch but matches if its title contains `-drama`; a query `"  "` (two spaces): sub-constraint trims to `""` and `"".contains` is true -> matches everything.
- Toolbar title/count rules (`getToolbarTitle`): fixture states for (showCategoryTabs, showCount) in the four combinations with a Default page and an 12-entry library: `(true,false)` -> ("Manga", None); `(true,true)` -> ("Manga", Some(distinct count)); `(false,false)` -> ("Default", None); `(false,true)` -> ("Default", Some(items in page)).

### 2.3 Categories (T-08a-007 to T-08a-012)

- Create on fresh DB: orders `0, 1, 2`; initial flags copy the global sort (`0x40` default; set global to `LastRead Desc` -> new category flags `0x04`).
- Create validation: `""`, `"   "` -> `InvalidName`; `" A "` stored as `"A"`; duplicate `"A"` -> `DuplicateName`; `"a"` allowed (case-sensitive).
- Delete sequence from section 1 of `p1-topic-category-lifecycle.md`: assert the exact `sort` values after each step (system row 0 after the first delete).
- Delete strips id from the five sets for both kinds (regression for Q8: manga exclude set gets cleaned) and resets `default_category` to -1 only when equal; a different default stays.
- Delete system category -> `DomainError::SystemCategory` and no row change; the DB trigger independently aborts a raw `DELETE` (test with raw SQL).
- Reorder: user [A, B, C] move C to index 0 -> orders `C 0, A 1, B 2`; unknown id -> `NotFound` (Kotlin `Unchanged`), system id -> `NotFound`; two concurrent reorders serialize (the second sees the first's result).
- `set_sort_mode`: `categorized_display = false` -> global preference + all categories updated; `= true` -> only the given category; `Random` re-rolls the seed (seed changes between two calls with overwhelming probability; inject a fake RNG to assert `store(seed)` called); `reset_flags` writes `global.flag()` to all categories including system.
- Membership: `set_entry_categories(e, [2,2,0,99])` -> rows `{2}` only (dedupe, system id dropped, nonexistent id dropped); `by_entry` ordered by `sort`; deleting a category cascades membership rows.
- Add-to-library target (`resolve_add_target`): table (default pref, categories) -> result: `(-1, [A,B]) -> Ask`; `(-1, []) -> Categories([])`; `(0, [A]) -> Categories([])`; `(B.id, [A,B]) -> Categories([B])`; `(999, [A,B]) -> Ask`.
- Bulk change: entries E1 in {A,B}, E2 in {B,C}; common = {B}, mix = {A,C}; add [D], remove [B] -> E1 {A,D}, E2 {C,D}; hidden-membership preservation: E1 also in hidden H -> H retained (Q26 fix).

### 2.4 History (T-08a-018 to T-08a-023)

- Upsert twice for chapter 7: `(t=1000, session=5000)` then `(t=2000, session=3000)` -> one row, `last_read = 2000`, `time_read = 8000`. Anime upsert twice -> one row, `last_seen` = second.
- Reset one: `last_read = 0`, row remains, `total_duration` unchanged; list excludes it. Reset entry: all rows of the entry. Clear all: rows gone, `total_duration = 0`, returns `true`.
- List query: the 4-row example of `p1-topic-history-and-next-item.md` section 2 incl. reveal-previous behaviour after a reset; ordering `readAt DESC`; query escaping: title `"100% Real"` with query `"100%"` matches only that title (after the escape fix); query `"%"` matches only titles containing a literal `%`.
- `latest()`: returns the single most recent row with `last_read > 0`; none -> `None`.
- Recorder: flush with no start -> `session = 0`; clamp negative -> 0; clamp > 12 h -> 12 h; incognito -> no DB call; anime recorder: 600 `tick()` calls in 10 s of fake time -> at most 3 upserts (0 s, 5 s, 10 s) plus the final flush; pause -> immediate upsert.
- `next_items` tables of `p1-topic-history-and-next-item.md` section 4 (all eight rows), for chapters sorted by number asc, by source order desc, by upload date, and by name; excluded-scanlator chapters never appear (manga) while anime has no scanlator exclusion; manga with no chapters -> `[]`; unknown manga id -> `[]`.
- `total_duration` with rows `0, 5000, 7000` and one reset row -> 12000.

### 2.5 Updates (T-08a-024, T-08a-025)

Fixture: favourite manga M (`date_added = 1_000`) with chapters a (`date_fetch 900`, `date_upload 5_000`), b (`2_000`, `5_000`), c (`3_000`, `100`), d (`4_000`, `6_000`, read); non-favourite manga N with chapter e (`date_fetch 9_000`, `date_upload 9_000`). Query `after = 4_000`: `a` is dropped because `date_fetch (900) <= date_added (1_000)`; `c` is dropped because `date_upload (100) <= after`; `e` is dropped (not favourite); remaining `b` and `d` are ordered by `date_fetch DESC`: result `[d, b]`. With the read filter `false`: `[b]`; with `true`: `[d]`. Limit 500 honoured with 600 synthetic rows (assert 500 rows, newest `date_fetch` first). Removing M from the library and re-adding it with `date_added = 3_500`: `b` (`date_fetch 2_000`) disappears, `d` stays.

`three_months_ago`: `2025-05-31T10:00 local` -> `2025-02-28T10:00`; `2024-05-31` -> `2024-02-29`; `2025-01-15` -> `2024-10-15`; DST gap handling (a local time that does not exist after subtraction) resolves to the next valid instant (Kotlin `ZonedDateTime` shifts forward by the gap length).

Date separators: sequence of timestamps at local `2025-03-02 23:59`, `2025-03-02 00:01`, `2025-03-01 12:00` -> headers before item 1 and item 3 only; zone change between runs not applicable.

### 2.6 Preferences (T-08a-026)

- A registry test enumerates the 87 accessors: key strings, default values, kind, namespace (exactly 3 app-state keys: `library_update_last_timestamp`, `last_used_anime_category`, `last_used_category`) match section 5 of `p1-data-and-config.md` (the test parses that table).
- Shared-key tests (Q1): writing `display_chapter_by_name_or_number` is observable through `display_episode_by_name_or_number`, and the same for direction.
- Swipe keys (Q2): defaults `swipe_left_chapter == ToggleRead` stored at key `pref_chapter_swipe_end_action`; `swipe_right_chapter == ToggleBookmark` at `pref_chapter_swipe_start_action`; episode `ToggleSeen`/`ToggleBookmark`.
- Unknown enum strings fall back to defaults without deleting the stored value (TriState `"BANANA"` -> `Disabled`, swipe `"Banana"` -> default).
- `set_chapter_settings_default(manga flags)` and episode/season twins copy exactly the masked components (property test: for random flag longs, `default` after set equals component extraction); regression for Q3 (season downloaded filter copied from downloaded bits).

## 3. Edge cases and failure modes

| # | Situation | Required behaviour |
|---|---|---|
| E1 | Library has entries but all are in hidden categories | Library shows the empty-library screen only if `distinct` count over *displayed* pages is 0 (it is), with no filters/search: "Your library is empty" even though entries exist (upstream behaviour). Proposal adds a hint line "N entries are in hidden categories" |
| E2 | `activeCategoryIndex` >= number of pages (category deleted/hidden) | clamp to `len-1`; with 0 pages use 0 |
| E3 | Category deleted while it is the selected tab | pager clamps; selection of entries of that tab is cleared |
| E4 | Entry in two categories selected from page 1 then page 2 | `Selection` stores one id; the page-2 row toggles the same selection (identity by entry id) |
| E5 | Search active and the category tab count | tab badges show match counts per category even if `display_number_of_items` is off |
| E6 | Sort set while a different category is active in the sheet | applies to the category passed by the sheet (the active tab at open time); if the active category disappears the sheet closes |
| E7 | Two quick sort taps on Random | two seeds, last wins; UI refreshes once because the pipeline coalesces |
| E8 | `categorized_display` switched on after a global sort of `UnreadCount Desc` | all categories already carry those flags; nothing changes visibly until a per-category change |
| E9 | History upsert for a chapter deleted between read and save | FK failure swallowed + logged; no UI error |
| E10 | History row whose chapter was reset to `last_read = 0` and then read again | upsert sets `last_read = now`, keeps accumulated `time_read`; the row reappears |
| E11 | Clock set backwards while reading | `last_read` may go backwards; `session_ms` clamped >= 0 |
| E12 | `GetNextChapters` for a manga whose configured sort type value is invalid (flags corrupted: masked value not in {0x000, 0x100, 0x200, 0x300}) | cannot happen: the 2-bit mask only yields those four values; the `NotImplementedError` branch in Kotlin is unreachable. Rust matches exhaustively on the 2-bit value |
| E13 | Updates feed with chapters whose `date_upload` is in the future | included (filter is `> after`), sorted by fetch date |
| E14 | Updates with a manga removed from the library | rows vanish immediately (view filters `favorite = 1`); counters are not decremented |
| E15 | Two Kurobayomi processes | prevented by the single-instance lock of area:01/02; if SQLite `SQLITE_BUSY` appears on a write, retry per `busy_timeout = 5000` (area:04) then report `DomainError::Db` |
| E16 | Backup restore while the library screen is open | the change bus emits per table; the pipeline coalesces 30 ms; the UI sees one or two updates |
| E17 | Category rename to the same name | no-op `Ok` (not `DuplicateName`) |
| E18 | Category with a very long name (e.g. 5,000 chars) | accepted by the domain (no limit upstream); the UI truncates with ellipsis; Proposal limit of 255 chars rejects with `InvalidName` |
| E19 | Preference set contains ids of categories that no longer exist (restored backup) | consumers ignore unknown ids; `CategoryPreferencesCleanupMigration`-style cleanup (`p1-tasks.md` T-08a-035) removes them on startup |
| E20 | Display mode string stored in lower case by a hand-edited config | treated as unknown -> `CompactGrid` |
| E21 | A manga appears twice in a category list because the database has a duplicate membership row (no unique constraint) | `distinct` by id in the view model; `set_entry_categories` always rewrites membership without duplicates |
| E22 | `history` query for an entry whose `thumbnail_url` is NULL | `CoverKey.url = None`; cover shows placeholder |
| E23 | Very large library (20,000 rows) | pipeline below 100 ms on a modern CPU, executed off the UI thread; UI replaces models incrementally per page (only the visible page is realised) |

## 4. Race conditions

1. **Rapid category reorder drags** (multiple `move_before` calls while a previous write is still running): serialized by the per-kind `Mutex`; each call re-reads the list under the lock (as upstream), so the final order equals the last call applied to the result of the previous ones.
2. **Delete vs reorder**: both take the same lock in Rust (Kotlin only locks reorder); the delete path acquires `reorder_lock` so renumbering cannot interleave with a reorder transaction.
3. **Library recompute vs write bursts**: `ChangeBus` events during a computation set the dirty flag; the computation result is published only if no newer snapshot exists, otherwise it is discarded and recomputed (equivalent of `collectLatest`).
4. **History flush on window close vs process exit**: the flush runs in a detached task but the process may exit first. `kuro-core` shutdown waits for registered `JoinHandle`s (max 2 s) before exiting.
5. **Category deleted while the change-category dialog is open**: confirm applies ids that no longer exist -> `set_entry_categories` drops nonexistent ids (FK would reject them).
6. **Preference change while pipeline runs**: the pipeline reads one `ItemPrefs` snapshot per run; the preference change event triggers a fresh run.

## 5. Fixtures and test infrastructure

- `kuro-testkit` (area:01) supplies `Db::in_memory(kind)` with the schemas of area:03/04 applied, `Prefs::in_memory()`, `FakeClock`, `FakeDownloadIndex`, `FakeSourceLookup`, `FakeTrackScores`.
- This area adds `kuro-domain/tests/fixtures/library_small.sql` (the 4-entry fixture of section 2.2 plus categories), `history_basic.sql`, `updates_window.sql` (the section 2.5 fixture), and `tests/golden/sort_orders.json` (per sort type and direction expected id orders for the 4-entry fixture; generated once by hand from the vectors above, then asserted).
- Property tests (`proptest`): `LibrarySort` flag/decoding round trip over random `i64`; `merge` idempotence `merge(merge(v,f,m),f,m) == merge(v,f,m)`; pipeline never panics for random rows/prefs and never changes the multiset of items except by filters/search; sort result is a permutation and is stable for equal keys.
- Golden compatibility test: a JSON export of `categories` + preference snapshot taken from a real Aniyomi install (if the user supplies one) is loaded and the decoded `LibrarySort` per category is compared with what the Android UI showed (manual verification checklist in T-08a-040).
