# Topic: source filters (model, state, browse flow, filter sheet)

Scope files: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Filter.kt`, `FilterList.kt`. Consumers: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/SourceFilterMangaDialog.kt`, `BrowseMangaSourceScreenModel.kt`, `BrowseMangaSourceScreen.kt`, `presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt`, `core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt`.

## 1. Model (OBSERVED, Filter.kt)

`sealed class Filter<T>(val name: String, var state: T)`. The state is mutable and owned by the filter object; the source creates the objects, the UI mutates them in place, and the source reads them when a search runs.

| Class | Constructor args (defaults) | state type | default state | Meaning |
|---|---|---|---|---|
| `Header` | `(name)` | `Any` (value `0`, unused) | `0` | Section heading row |
| `Separator` | `(name = "")` | `Any` (`0`, unused) | `0` | Divider row |
| `Select<V>` | `(name, values: Array<V>, state: Int = 0)` | `Int` index | `0` | Dropdown; `values` are displayed via `toString()` |
| `Text` | `(name, state: String = "")` | `String` | `""` | Free text field |
| `CheckBox` | `(name, state: Boolean = false)` | `Boolean` | `false` | Checkbox |
| `TriState` | `(name, state: Int = STATE_IGNORE)` | `Int` 0/1/2 | `0` (ignore) | Three-state toggle |
| `Group<V>` | `(name, state: List<V>)` | `List<V>` | none (required) | Collapsible container of nested filters |
| `Sort` | `(name, values: Array<String>, state: Selection? = null)` | `Selection?` | `null` | Collapsible list with ascending/descending |

`Sort.Selection(index: Int, ascending: Boolean)` is a Kotlin data class.

TriState constants (Filter.kt:17-21): `STATE_IGNORE = 0`, `STATE_INCLUDE = 1`, `STATE_EXCLUDE = 2`; helpers `isIgnored()`, `isIncluded()`, `isExcluded()` compare `state`.

Equality (Filter.kt:31-42): equal when `name` and `state` are equal (`state` compared with `==`; `Selection` is a data class, lists compare by element). `hashCode = 31 * name.hashCode() + (state?.hashCode() ?: 0)`.

`FilterList` (FilterList.kt): `data class FilterList(val list: List<Filter<*>>) : List<Filter<*>> by list`; `equals` always returns `false` (so the UI always treats two lists as different); `hashCode = list.hashCode()`.

Only these eight kinds exist; a source cannot declare another filter kind. `Group` may contain any kind, including nested `Group`s (the UI renders them recursively, see section 5.8).

## 2. Creation and lifetime (OBSERVED)

- `CatalogueSource.getFilterList()` is called:
  - in the model `init` (BrowseMangaSourceScreenModel.kt:87-95): when the screen opens, the current `Listing.Search` gets a new `FilterList` from the source, and `state.filters` is set to a fresh list;
  - in `resetFilters()` (line 147-151): `filters = source.getFilterList()`;
  - in `search(...)` when no `Listing.Search` exists yet (line 171);
  - in `searchGenre` (line 185, `defaultFilters = source.getFilterList()`).
- Each call must return NEW objects (the source returns fresh instances; HttpSource default returns `FilterList()`, HttpSource.kt:460).
- A source that returns the same objects twice would share state between the listing and the sheet (not guarded by the app).

## 3. Browse search flow (OBSERVED)

### 3.1 Listing states
`BrowseMangaSourceScreenModel.Listing` (sealed, line 331):
- `Popular` — `query = GetRemoteManga.QUERY_POPULAR` = `"eu.kanade.domain.source.manga.interactor.POPULAR"`, `filters = FilterList()`.
- `Latest` — `query = QUERY_LATEST` = `"eu.kanade.domain.source.manga.interactor.LATEST"`, `filters = FilterList()`.
- `Search(query: String?, filters: FilterList)`.
- `valueOf(query)`: a sentinel string maps to `Popular`/`Latest`; anything else (including null) maps to `Search(query, FilterList())` ("filters are filled in later").

Remote paging (`MangaSourcePagingSource.kt`, see `p1-topic-models-and-sync.md` section 6): `page` starts at **1**; `load` calls `getSearchManga(page, query, filters)`, `getPopularManga(page)` or `getLatestUpdates(page)`; an empty `mangas` list is an error `NoChaptersException` (shown as "No results found"); `nextKey = page + 1` when `hasNextPage`; `pageSize = 25` for the Pager (display paging, not the source page size).

### 3.2 `search(query, filters)` (lines 167-182)
1. Return if `source !is CatalogueSource`.
2. `input = state.listing as? Listing.Search ?: Listing.Search(null, source.getFilterList())`.
3. New listing: `Listing.Search(query = query ?: input.query, filters = filters ?: input.filters)`; `toolbarQuery = query ?: input.query`.
4. Filters are applied by the paging source at load time with the listing's `filters` object (the same mutable objects as in the sheet).

### 3.3 `searchGenre(genreName)` (lines 184-227) — exact algorithm
Used by "search this genre" actions (for example from a manga's genre chip).
1. Return if not `CatalogueSource`.
2. `defaultFilters = source.getFilterList()`; `genreExists = false`.
3. For each top-level filter `sourceFilter` in `defaultFilters` (labelled loop `filter@`):
   - If it is a `Group`: for each nested `filter` in `sourceFilter.state` that is a `Filter<*>` with `filter.name.equals(genreName, ignoreCase = true)`:
     - `TriState` → `state = 1` (INCLUDE); `CheckBox` → `state = true`; any other kind → no change (but `genreExists` is still set);
     - `genreExists = true`; break out of `filter@` (both loops).
   - Else if it is a `Select`: `index = values.filterIsInstance<String>().indexOfFirst { it.equals(genreName, true) }`; if `index != -1`: `state = index`, `genreExists = true`, break the outer loop.
     - Note: `filterIsInstance<String>()` removes non-string values first, so the index is computed in the FILTERED list but written as the state of the UNFILTERED `values` array. Differs when a `Select` has non-string values (observed; Kurobayomi uses `Vec<String>` so the mismatch cannot occur).
   - `Sort`, `Text`, `CheckBox`, `TriState`, `Header`, `Separator` at top level are ignored by this algorithm.
4. Result: `filters = defaultFilters`; if `genreExists` → `Listing.Search(query = null, filters = defaultFilters)`; else → `Listing.Search(query = genreName, filters = defaultFilters)` (a plain text search for the genre name). `toolbarQuery = listing.query`.

Kurobayomi (PROPOSAL, `p1-tasks.md` T-15a-040): identical algorithm; the `Select` branch uses `values` directly (strings) so there is no index mismatch.

### 3.4 Sheet actions
- Filter button (`onFilter`): `screenModel.search(filters = state.filters)` (BrowseMangaSourceScreen.kt:260), then dismiss the sheet.
- Reset button (`onReset`): `screenModel.resetFilters()` (BrowseMangaSourceScreen.kt:259) — replaces ALL filter objects with `source.getFilterList()`. It does NOT re-run the search. The sheet shows the reset state at once; the listing keeps the old filters until Filter is pressed.
- Each change in the sheet: `onUpdate(filters)` → `setFilters(filters)` (line 157-165) → updates `state.filters` only (no search). Filters take effect on Filter press.

## 4. Filter sheet layout (OBSERVED, SourceFilterMangaDialog.kt)

- `AdaptiveSheet(onDismissRequest)` containing a `LazyColumn`.
- `stickyHeader`: a row with padding 8 dp on `MaterialTheme.colorScheme.background`:
  - left: `TextButton` with text `MR.strings.action_reset` ("Reset"), colour `primary`;
  - spacer (weight 1);
  - right: `Button` with text `MR.strings.action_filter` ("Filter"); click → `onFilter()` then `onDismissRequest()`.
  - followed by a `HorizontalDivider()`.
- Body: `items(filters) { FilterItem(it, updateFilters) }`. `updateFilters = { onUpdate(filters) }`.
- The top-level list is rendered in source order; no grouping, sorting or filtering of items.

## 5. Per-kind rendering and state transitions (OBSERVED)

`FilterItem(filter, onUpdate)` (private composable) — `when (filter)` with eight branches:

### 5.1 Header
`HeadingItem(filter.name)`. No state change.

### 5.2 Separator
`HorizontalDivider()`. No text is shown; `filter.name` is not displayed.

### 5.3 CheckBox
`CheckboxItem(label = filter.name, checked = filter.state) { filter.state = !filter.state; onUpdate() }`.

### 5.4 TriState
- Display: `TriStateItem(label = filter.name, state = filter.state.toTriStateFilter())`.
- Mapping (`Int.toTriStateFilter()`, lines 167-174): `0 → DISABLED`, `1 → ENABLED_IS`, `2 → ENABLED_NOT`; any other value throws `IllegalStateException("Unknown TriState state: $this")`.
- Click: `filter.state = filter.state.toTriStateFilter().next().toTriStateInt()`; `onUpdate()`.
- Cycle order (core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt:9-15): `DISABLED → ENABLED_IS → ENABLED_NOT → DISABLED`, i.e. `0 → 1 → 2 → 0`.
- `TriState.toTriStateInt()` (lines 176-181): `DISABLED → 0`, `ENABLED_IS → 1`, `ENABLED_NOT → 2`.

### 5.5 Text
`TextItem(label = filter.name, value = filter.state) { filter.state = it; onUpdate() }`. `TextItem` binds `onValueChange = onChange`, so the state is written and `onUpdate` called on EVERY keystroke (no debounce, no commit on focus loss).

### 5.6 Select
`SelectItem(label = filter.name, options = filter.values, selectedIndex = filter.state) { filter.state = it; onUpdate() }`.
- `SelectItem` (SettingsItems.kt:302-330) shows a read-only `OutlinedTextField` with `value = options[selectedIndex].toString()` and an expanded menu of options. `selectedIndex` out of range → `IndexOutOfBoundsException` at composition time (observed; Kurobayomi validates in `Filter::select_value()`).
- Selection writes the chosen index.

### 5.7 Sort
`CollapsibleBox(heading = filter.name) { Column { values.mapIndexed { index, item -> SortItem(label = item, sortDescending = …) { … } } } }`.
- `sortDescending = filter.state?.ascending?.not()?.takeIf { index == filter.state?.index }`. Result: `true` (descending icon) only for the selected index when the selection is descending; `false` (ascending icon) for the selected index when ascending; `null` (no icon) for all other rows and for `state == null`.
- Click handler:
  ```
  ascending = if (index == filter.state?.index) !filter.state!!.ascending else filter.state!!.ascending
  filter.state = Filter.Sort.Selection(index = index, ascending = ascending)
  onUpdate()
  ```
- Behaviour (OBSERVED):
  - Tapping the currently selected row toggles its direction.
  - Tapping another row selects it and KEEPS the current direction.
  - When `state == null`, `index == filter.state?.index` is false, so the code evaluates `filter.state!!.ascending` and throws `NullPointerException`. A `Sort` created with `state = null` cannot be tapped without crashing (observed hazard). Kurobayomi defines: a `Sort` with `None` selection selects the tapped row with `ascending = true` (PROPOSAL; documented change, `p1-tasks.md` T-15a-010).
- Note: `filter.state!!` is evaluated only when a row is tapped.

### 5.8 Group
`CollapsibleBox(heading = filter.name) { Column { filter.state.filterIsInstance<Filter<*>>().map { FilterItem(filter = it, onUpdate = onUpdate) } } }`.
- Nested items render with the same `FilterItem`; a nested `Group` nests again.
- Items in the group's list that are not `Filter` instances are skipped silently.
- The default expanded or collapsed state of `CollapsibleBox` (presentation-core) was not read for this area. Verify it in `presentation-core` before porting; Kurobayomi uses the same default.

## 6. Searching uses the same objects

`Listing.Search(query, filters)` keeps the `FilterList` object of the sheet. Changes made in the sheet after a search do not alter the running listing until Filter is pressed again (`search(filters = state.filters)` creates a new `Listing.Search` with the same object reference; the paging source is re-created when the listing changes, because `mangaPagerFlowFlow` maps `state.listing`).

## 7. What the source must do (contract)

1. Read the filter objects passed to `getSearchManga(page, query, filters)`; read `state` of each kind as documented in section 1. Read `Select` as an index into `values`; `Sort` as `Selection?`; `Group` recursively.
2. Do not store the `FilterList` beyond the call.
3. Treat `Header`/`Separator` as having no value.
4. A `Sort` with `state == null` means "no explicit sort"; the source applies its default.
5. TriState `2` means exclude; `1` include; `0` ignore.

## 8. Kurobayomi design (PROPOSAL)

- `enum Filter` (in `kuro-source::filter`) as in `p1-rust-design.md` section 5.
- Sheet renderer (`kuro-app`, GTK): a `gtk::ListBox` inside a `gtk::Window`/`adw`-free sheet (GTK 4 only; no libadwaita). Header with Reset and Filter buttons (`gtk::Button` with `action_reset`, `action_filter` labels); body rows per kind:
  - Header → `gtk::Label` heading row (bold via CSS class `heading`);
  - Separator → `gtk::Separator`;
  - CheckBox → `gtk::CheckButton`;
  - TriState → a button showing the current state label; click cycles `0 → 1 → 2 → 0`; keyboard: Space/Enter activates;
  - Text → `gtk::Entry` with `changed` signal; the change is written on every keystroke (observed behaviour) — PROPOSAL: keep it, but coalesce the search trigger, not the state write;
  - Select → `gtk::DropDown` with `StringList` built from `values`;
  - Sort → an `gtk::Expander` with one row per value; arrow icon from `go-up-symbolic` / `go-down-symbolic` in the selected row; click rule per 5.7 with the None-state fix;
  - Group → `gtk::Expander` with the nested rows; recursive.
- Reset and Filter buttons: same semantics as 3.4.
- Open the sheet with a `gtk::Window` (modal, transient for the main window) on desktop; mobile sheets are not relevant.

## 9. Fixtures for T-15a-048

- `Select` with values `["A","B","C"]`, state `2` → display `C`; state `5` → no crash, displays nothing, and `select_value()` returns `None`.
- `TriState` raw 3 → error `Parse`, never a panic.
- `Sort` values `["Popularity","Latest","Title"]`, state `Selection(index=1, ascending=true)`: tapping index 1 → `Selection(1, false)`; tapping index 0 → `Selection(0, true)`.
- `searchGenre("action")` with a `Group` containing `TriState "Action"` → state 1, listing filters = defaults, query `null`; with no match → query `"action"`.
- Equality: two lists with identical names and states compare equal in Rust (Kotlin `FilterList.equals` would return false; Rust deliberately differs, documented).
