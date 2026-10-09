# Topic — Library pipeline: filter, sort, search (deep dive)

Companion to p1-behavior.md §B-2 to §B-6 and p1-rust-design.md §R-4, §R-5, §R-7. Upstream paths are repo-relative to `/home/blackbox/aniyomi-src/`.

## 1. Order of operations (must match)

Upstream `applyFilters(...).applySort(...)` then search, per category (`app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt:115-125`):

1. Decorate badges (`getLibraryFlow`, `:379-416`). Badge values do not affect filtering except `unseenCount` (used by the continue button, E-02) and `downloadCount` (filter uses the cache directly).
2. Group by `category` (view column).
3. Choose display categories (`:407-415`).
4. Filter: AND of six predicates plus tracker predicate (`:243-251`).
5. Sort: per category, using `key.sort` (`:329-339`).
6. Search: per category, after sort (`:119-125`).
7. Derive `hasActiveFilters` from raw preferences (`:153-175`).

Why this order matters:
- Search after sort keeps the sorted order of matches, so the Random shuffle is applied to the full category, then filtered (matches shuffle positions of the full list). Changing search does not reshuffle.
- Filtering before sorting makes the "Random" permutation depend on the filtered set. Changing a filter changes the shuffle, since `shuffled(Random(seed))` depends on list length.

## 2. Worked example — sort

Library category with 5 rows (title, unseen, lastSeen):

| Title | unseen | lastSeen |
|---|---|---|
| Bleach | 0 | 900 |
| akira | 3 | 100 |
| Berserk | 1 | 500 |
| Chainsaw | 0 | 0 |
| Dune | 2 | 700 |

- Alphabetical ascending (collation on lowercased): akira, Berserk, Bleach, Chainsaw, Dune.
- UnseenCount ascending: unseen values `[1 (Berserk), 2 (Dune), 3 (akira)]` first, then zeros in alphabetical order: Berserk, Dune, akira, Bleach, Chainsaw. (Zeros last. Within equal unseen, the tiebreak is alphabetical ascending.)
- UnseenCount descending: `[3 akira, 2 Dune, 1 Berserk]`, then zeros: Bleach, Chainsaw. Zeros still last.
- LastSeen descending: Bleach (900), Dune (700), Berserk (500), akira (100), Chainsaw (0). Zero is last because it is the smallest value, not because of a zero rule.

Test vector: T2-SORT-03.

## 3. Worked example — tracker mean

Entries A, B, C, D; logged-in tracker id 1 only.
- A: tracks on tracker 1 with score 7.5 → mean 7.5.
- B: tracks on tracker 2 (logged out) only → upstream list empty → NaN.
- C: no tracks → `trackMap[C]` absent → `null` → −1.0.
- D: tracks on tracker 1 with score 9.0 → 9.0.

Upstream ascending: C (−1.0), A (7.5), D (9.0), then B (NaN sorts above all numbers). Upstream descending: B (NaN first), D, A, C.

Rust rule: B becomes `None` → −1.0, so B and C tie and fall to the alphabetical tiebreak. Test: T2-SORT-06.

## 4. Filter truth table (upstream semantics)

`TriState::apply(pred)`: Disabled → true; Is → pred; Not → !pred.

| Filter | Predicate |
|---|---|
| downloaded | `isLocal \|\| downloadCount > 0 \|\| cache.count > 0` (`:198-204`); `Is` forced when downloaded-only is on |
| unseen / unread | `unseenCount > 0` (raw total − seen) |
| started | `seenCount > 0` |
| bookmarked | `bookmarkCount > 0` |
| completed | `status == 2` |
| interval custom | only when restriction `manga_outside_release_period` on: `fetchInterval < 0`; else ignored |
| tracked (per tracker) | see §5 |

Common confusion: `Not` on "completed" shows ongoing titles; `Not` on "downloaded" shows entries with no local copy and no download count.

## 5. Tracker filter

Inputs: `trackingFilter: Map<trackerId, TriState>` (logged-in trackers only), `trackMap: entryId → [trackerId]` (all tracks).

```
excluded = keys where Not
included = keys where Is
if trackingFilter empty or (included empty and excluded empty) → pass
isExcluded = excluded nonempty and entryTrackers ∩ excluded nonempty
isIncluded = included empty or entryTrackers ∩ included nonempty
pass = !isExcluded && isIncluded
```
Note: an entry with no tracks fails any `Is` filter and passes any `Not` filter.

Performance: upstream rebuilds `trackMap.mapValues` per row (`:233-234`), O(n²). Rust precomputes the include/exclude lists once and looks up by id (`TrackerLists`, design R-4).

## 6. Search grammar

```
query      := "id:" integer              (case-insensitive prefix; anything else after prefix must parse as i64)
            | terms
terms      := term ("," term)*           (comma split, each trimmed)
term       := "-" text | text            (negation applies to the whole term after "-", then left-trimmed)
```
Evaluation:
```
if query is id form: row.id == parsed
else:
  if contains_ci(title|author|artist|description, whole_query): match
  else: all terms satisfy:
     plain x  → contains_ci(source_name, x) || any genre == x (ci)
     negated x → !(contains_ci(source_name, x) || any genre == x (ci))
```
Quirks kept on purpose (see E-03..E-06): empty query matches all; a lone "-" matches only whole-string hits; `"a,,b"` ignores the empty token.

Source name: `getNameForAnimeInfo()` of the entry's source (area:17). Resolved once per source per recompute.

## 7. Random

- Seed: `library_random_*_sort_seed`, i32, default 0 (upstream). Reseeded only on choosing Random (`SetSortModeForAnimeCategory.kt:23-25`).
- Upstream applies `shuffled(Random(seed))` to the **filtered** list (the filter runs first, §1).
- Rust: `items.shuffle(&mut StdRng::seed_from_u64(seed as u64))`. Sequence differs from Kotlin; acceptable (no shared state).
- Same seed across categories (E-21).

## 8. Decision table

| Question | Decision | Task |
|---|---|---|
| Search: keep whole-string path before comma split? | Yes (parity) | T-37a-012 |
| Empty query means "all" or "no search"? | Both behave as all; UI normalises "" to None | T-37a-012 |
| Continue button tied to unread badge? | No (fix) | T-37a-033 |
| TrackerMean NaN | `-1.0` (fix) | T-37a-058 |
| Zero-last in both directions | Keep (parity) | T-37a-009 |
| Default category initial sort Z→A | Keep for backup compatibility; migrate later | T-37a-002 |
| hasActiveFilters with downloaded-only forced | Include (fix) | T-37a-056 |
| Random sequence parity | Not required | T-37a-011 |
| Performance: tracker map rebuild | Precompute (fix) | T-37a-054 |
