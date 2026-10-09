# Area 40 (part 2) topic: browse listing, paging and the filter tree

Covers `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt` (370 lines), `BrowseMangaSourceScreen.kt` (337), `SourceFilterMangaDialog.kt` (181), and the paging adapter `data/src/main/java/tachiyomi/data/source/manga/MangaSourcePagingSource.kt`. Algorithms are in `p2-behavior.md` sections 4 to 6. This file defines the listing transitions, the generation rule that replaces Kotlin equality, and the filter tree semantics with worked examples.

## 1. Listing transitions

Listing values: `Popular`, `Latest`, `Search(query: String?, filters)`. Each transition below names the code that performs it and what it does to the paging generation.

| From | Action | To | Filters after | Toolbar text after | Generation | Code |
|---|---|---|---|---|---|---|
| any | open with nav `Popular` | Popular | source defaults (state only) | none | 0 | `BrowseMangaSourceScreenModel.kt:75,81-98` |
| any | open with nav `Latest` | Latest | source defaults (state only) | none | 0 | same |
| any | open with nav `Search(q)` | Search(q, defaults) | source defaults | q | 0 | `:87-97` |
| Search(q,f) | chip Popular | Popular | source defaults | none | +1 | `BrowseMangaSourceScreen.kt:158-162` (reset then set) |
| Popular | chip Latest | Latest | source defaults | none | +1 | `:177-181` |
| any | toolbar submit `s` | Search(s, current or defaults) | current if Search, else defaults | s | +1 | `search(query=s)` `:167-182` |
| any | Filter button with values `v` | Search(current query, v) | v | current query | +1 | `search(filters=state.filters)` `BrowseMangaSourceScreen.kt:260` |
| any | genre `g` found in tree | Search(null, tree-with-g-set) | tree | `null` (toolbar shows nothing) | +1 | `searchGenre` `:184-227` |
| any | genre `g` not found | Search(g, defaults) | defaults | g | +1 | same |
| any | `setToolbarQuery(t)` | unchanged | unchanged | t | 0 | `:327-329` |
| any | `setFilters(v)` | unchanged | v | unchanged | 0 | `:157-165` |
| any | `resetFilters()` | unchanged | defaults | unchanged | 0 | `:147-151` |

Rule: generation increases on every action that Kotlin turns into a new `Listing.Search` object or a new `Popular`/`Latest` selection. Because `Search` never equals another `Search` (`FilterList.equals` is always false), Kotlin's `distinctUntilChanged` never dedupes them. So every such action restarts the pager. `setToolbarQuery` and `setFilters` keep the same listing object, so Kotlin does not restart; the table reflects that.

Edge: choosing Popular while already on Popular emits the same singleton, so `distinctUntilChanged` dedupes it and the list does not restart. The filters are still reset (`resetFilters()` runs first, `BrowseMangaSourceScreen.kt:160-161`). Rust: `set_listing` bumps the generation only when the new listing differs from the current one, or when it is a `Search` (`p2-rust-design.md` section 3.5).

## 2. Worked example: chip, filter, toolbar

Source S with filters `[Group("Genres", [CheckBox("Action"), TriState("Drama")]), Sort("Order", ["Latest","Name"], None)]`, `supportsLatest = true`.

1. Open S with nav `Popular`. Generation 0. Filters = defaults (copy 1). Filter chip shows (filters non-empty). Popular chip selected.
2. Click Filter chip: sheet opens (`Dialog::Filter`). Tick "Action": `CheckBox.state = true`, `setFilters(copy)`, no search.
3. Click Sort "Name" (index 1): `sort_click(None, 1)` = `{index 1, ascending true}` (D-BR-01). No search.
4. Click Filter button: `search(filters = state.filters)`. Listing becomes `Search(None, {Action=true, Sort=Name asc})`. Generation 1. Toolbar empty. Sheet closes. Filter chip selected; Popular unselected.
5. Type "one" in toolbar and submit: `search(query = "one")`. Listing `Search("one", {Action, Sort})` (filters kept, since the listing is Search). Generation 2. Toolbar "one".
6. Click Popular chip: `resetFilters()` -> filters = defaults (Action unchecked, Sort None). `setListing(Popular)` -> toolbar cleared. Generation 3.
7. Click Filter chip (now shows defaults, Sort None). Click Sort "Latest" (index 0) with state None: ascending true. Click Sort "Latest" again: flips to descending (`sort_click(Some{0,true}, 0)` -> `{0, false}`).

Note step 5: the filters carried into the search are the ones from the Search listing (`:170-178`), not the fresh defaults, because the listing is already Search.

## 3. Navigate-up sequence

| State | Back pressed | Result |
|---|---|---|
| Popular, toolbar null | pop | leave page |
| Search("one"), toolbar "one" | `isUserQuery` true -> pop | leave page |
| Search(null, f), toolbar "two" (typed, not submitted) | `isUserQuery` false and toolbar non-null -> clear toolbar | stay; listing unchanged |
| then back again | toolbar null -> pop | leave page |

Rust: `BrowseSourceState::navigate_up` implements this table (`p2-rust-design.md` section 3.5).

## 4. Paging (demand driven)

Page size 25; initial window 75 (library default, not overridden in Kotlin). Pages are requested in order starting at 1.

| Event | Rust PageState before | Action | PageState after |
|---|---|---|---|
| page created | Initial | demand page 1 | Loading{1} |
| page 1 ok, 25 items, hasNext | Loading{1} | append; next = 2 | Idle{next: Some(2)} |
| prefetch (last visible 40 of 50 loaded, 40+25 >= 50) | Idle{Some(2)} | demand 2 | Loading{2} |
| page 2 returns 0 items | Loading{2} | append error NoResults | ErrorAppend{2, NoResults} |
| user taps retry | ErrorAppend{2,..} | demand 2 | Loading{2} |
| page 3 returns 4 items, hasNext false | Loading{3} | append; next None | EndOfList |
| page 1 returns 0 items (empty source) | Loading{1} | ErrorFirst(NoResults) | ErrorFirst(NoResults) |
| page 1 source error | Loading{1} | ErrorFirst(Source(msg)) | ErrorFirst(Source) |

Consequence for the UI: an append error keeps the items already shown (part 1 error bar). A first-page error shows the empty error state.

Generation: each listing transition creates a new loader with the new generation. Events with an older generation are dropped (E-PG-03).

## 5. Row mapping (the favorite flag)

Each page item goes through `networkToLocalManga` before display (`BrowseMangaSourceScreenModel.kt:115-120`). The local row becomes the row's live model. The favorite flag comes from the database row, so:
- Adding a manga from the row's long-press updates the row's favorite badge with no page reload (the row observes its database row, area 05 change notifications).
- `hide_in_library_items` is evaluated when a page is shown (`:121`). A favorite added after the page is shown stays visible until the listing changes (B-EC-15).

## 6. Filter tree semantics

Filter kinds and their raw values (source-api `Filter.kt`):

| Node | Stored | Click or change | Notes |
|---|---|---|---|
| Header | none | none | heading row |
| Separator | none | none | divider |
| CheckBox | Boolean | toggle | |
| TriState | raw 0/1/2 | cycle 0 -> 1 -> 2 -> 0 | raw 3+ invalid (D-BR-02) |
| Text | String | every keystroke | no search |
| Select | index into values | select dropdown item | clamp on read (B-EC-21) |
| Sort | Option<(index, ascending)> | click item: flip if same index, else keep direction | None -> ascending on first click (D-BR-01) |
| Group | children | children act themselves | recursive |

### 6.1 Sort arrows

For item `i` with state `Some{index, ascending}`:
- `i == index && ascending == true` -> arrow up.
- `i == index && ascending == false` -> arrow down.
- otherwise -> no arrow.

Matches `SourceFilterMangaDialog.kt:132-135` (`sortDescending = !ascending` when `i == index`).

### 6.2 Genre search worked example

Tree: `[Select("Status", ["Any", "Ongoing"], state 0), Group("Genres", [TriState("Action", raw 0), CheckBox("Romance", false)])]`.

| Query | Walk | Result |
|---|---|---|
| "action" | top `Select` no string match; `Group` child `TriState("Action")` matches case-insensitively -> raw 1 | found; Search(None, tree) |
| "romance" | `Group` child `CheckBox` matches -> true | found |
| "ongoing" | `Select` string value "Ongoing" index 1 -> state 1 | found (top-level Select branch) |
| "ended" | no match | not found -> Search("ended", defaults) |
| "Any" | `Select` index 0 matches; state set to 0 (already 0) | found |

Kotlin's first-match rule: the walk stops at the first match; `break@filter` exits the outer loop (`BrowseMangaSourceScreenModel.kt:199-200`). With the tree above, "action" matches inside the Group, so the Select is not examined.

### 6.3 Select index quirk (D-BR-03)

Values `[SelectValue("A", string), SelectValue("B", non-string), SelectValue("C", string)]`. Query "C":
- Kotlin: `filterIsInstance<String>()` gives `["A","C"]`, index of "C" = 1, so state = 1, which displays "B" (wrong).
- Rust: position in `values` = 2, state = 2, displays "C".

## 7. Dialog state machine

| Dialog | Opened by | Closed by | Writes |
|---|---|---|---|
| Filter | chip Filter | Filter button (search then close), outside click | filters (state), search on Filter button |
| RemoveManga | long press on favorite | confirm (favorite remove) or cancel | favorite |
| AddDuplicateManga | long press, duplicate exists | confirm (add anyway), open (page), migrate (next dialog) | favorite on confirm |
| ChangeMangaCategory | add, default -1 with categories | confirm (favorite + links) or cancel | favorite + category links |
| Migrate | from AddDuplicateManga "Migrate" | migrate dialog close | migrate writes (area 41 scope) |

Only one dialog at a time (single `dialog` field). A new dialog replaces the open one (B-EC-27).

## 8. Test vectors

| Vector | Input | Expected |
|---|---|---|
| L1 | `Listing::from_nav(Search(None), defaults)` | `Search{query: None, filters: defaults}` |
| L2 | Popular chip while Popular | generation unchanged |
| L3 | `search(Some("x"), None)` from Popular | listing Search("x", defaults), toolbar "x", generation +1 |
| L4 | navigate-up table of section 3 | pop, stay, pop as listed |
| P1 | load_outcome(1, Ok(empty)) | Err(NoResults) |
| P2 | `should_prefetch(40, 50)` | true |
| P3 | `should_prefetch(10, 50)` | false |
| F1 | `sort_click(None, 1)` | {1, true} |
| F2 | `sort_click(Some{1,true}, 1)` | {1, false} |
| F3 | `TriState::from_raw(3)` | Err(InvalidTriState(3)) |
| F4 | `select_text(values, 9)` with 2 values | values[0] (clamp) |
| G1 | Section 6.3 example, query "C" | state 2 (Rust) |
