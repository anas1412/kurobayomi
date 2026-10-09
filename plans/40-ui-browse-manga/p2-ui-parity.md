# Area 40 (part 2): UI parity for the browse models' screens

Scope of this file: the screen wrappers and dialogs owned by the 12 part-2 files (`p2-behavior.md` section 0 lists them). Visual layout, grids, rows, cards and the header bar of the browse page are specified in part 1 (`p1-ui-parity.md` sections 3 to 6 and 10). This file adds what the part-2 code decides: which state is shown when, which dialog opens, what each control does, and what the mockup lacks.

Where this file and part 1 disagree, the code (cited here) is the authority, and the conflict is listed in section 9.

Notation: `B§n` = `p2-behavior.md` section n. `P1§n` = `p1-ui-parity.md` section n.

---

## 1. Screen inventory (part-2 screens)

| # | Screen | Kotlin wrapper | Opens from | Part-1 layout ref |
|---|---|---|---|---|
| S-A | Installed sources (Sources page) | `MangaSourcesTab.kt` (`mangaSourcesTab()`) | Browse tab sub-tab "Manga Sources" | P1§3, P1§4 |
| S-B | Sources options dialog | `MangaSourcesTab.kt:58-76` | long press on a source row | P1§4.3 |
| S-C | Show/hide sources | `MangaSourcesFilterScreen.kt` | Sources page action "Filter" | P1§5 |
| S-D | Browse one source | `BrowseMangaSourceScreen.kt` | Sources row (Popular), Latest button, global search row, genre tag | P1§3 |
| S-E | Browse filter sheet | `SourceFilterMangaDialog.kt` | S-D chip "Filter" | this file §3 |
| S-F | Global search | `GlobalMangaSearchScreen.kt` | Sources action "Global search", manga page title search, library search, deep links | P1§10 |
| S-G | Migrate: pick old source | `MigrateMangaSourceTab.kt` | Browse tab sub-tab "Migrate" | P1§6.1 |
| S-H | Add-to-library duplicate dialog | `app/src/main/java/eu/kanade/presentation/entries/manga/DuplicateMangaDialog.kt` (used at `BrowseMangaSourceScreen.kt:264-275`) | long press on a row whose title matches a favorite | this file §4 |
| S-I | Remove-from-library confirm | `RemoveEntryDialog` (P1 S19) used at `BrowseMangaSourceScreen.kt:289-297` | long press on a favorite row | P1 S19 |
| S-J | Category chooser | `ChangeCategoryDialog` used at `BrowseMangaSourceScreen.kt:298-311` | add to library, default category "Always ask" | area 08 |
| S-K | Migrate confirm (from duplicate) | `MigrateMangaDialog.kt` used at `BrowseMangaSourceScreen.kt:277-288` | S-H "Migrate" | P1§6.5 |

---

## 2. States per screen

### 2.1 S-A Sources page (state = `MangaSourcesScreenModel.State`, B§2.4)

| Model state | Visible | Notes |
|---|---|---|
| `isLoading = true` (initial) | spinner, full area | Kotlin: `LoadingScreen` (P1§3.1) |
| `isLoading = false`, `items` empty | empty text `source_empty_screen` ("No source found") | Also shown during the pre-init window (B§2.4 race). Proposal D-SS-01: keep the spinner until `MangaSourceManager.isInitialized` |
| `items` non-empty | headers + rows | P1§3.1 |
| `FailedFetchingSources` event | snackbar/toast `internal_error` ("InternalError: Check crash logs for further information") | channel is buffered (`Channel(Int.MAX_VALUE)`); a toast shows once per event |
| `dialog != null` | options dialog (S-B) | |

Interaction rules (code-verified):
- Row click: `Listing.Popular` (`MangaSourcesScreen.kt:124`). Latest button: `Listing.Latest` (`:127-128`) only when `supportsLatest`.
- Pin button (on the row): toggles pin immediately, no confirmation, no toast (`MangaSourcesScreen.kt:139`; `MangaSourcesTab.kt:54`).
- Long press: `showSourceDialog`.
- The local source row has no "Disable" action (P1§4.3); pin is allowed.

### 2.2 S-B Options dialog (B§2.5)

Rows, in this order (`MangaSourcesTab.kt:58-76`):
1. "Unpin" when `Pin.Pinned in source.pin`, else "Pin". Click = toggle pin, then close.
2. "Disable" (hidden for `LocalMangaSource.ID`). Click = toggle disable, then close.
3. Data saver row: dropped (B§15). Not rendered.

No confirm button, dismiss on outside click or Escape (`onDismiss = closeDialog`).

### 2.3 S-C Sources filter (state = `MangaSourcesFilterScreenModel.State`, B§3.1)

| State | Visible | Behavior |
|---|---|---|
| `Loading` | `LoadingScreen` | |
| `Error(throwable)` | nothing rendered; toast `internal_error` and the screen pops | `MangaSourcesFilterScreen.kt:30-37`. No error view. |
| `Success` with empty `items` | `EmptyScreen` with `source_filter_empty_screen` ("No installed source found") | P1§4 |
| `Success` | list: language header switch + per-source checkbox rows | P1§4 |

Controls and effects:
- Language header switch: `ToggleLanguage` (B§3.3). When off, that language's sources are not rendered (no per-source rows).
- Source row (click or checkbox): `ToggleMangaSource` (B§3.3). Checkbox checked = visible (`"${id}" !in disabledSources`).
- Order inside a language: enabled first, then name (B§3.2 step 3).
- Language order: enabled languages first, then `LocaleHelper.comparator` ("all" first among them) (B§3.2 step 4).

### 2.4 S-D Browse one source (state = `BrowseMangaSourceScreenModel.State` + paging state, B§4, B§5)

Header and chips (`BrowseMangaSourceScreen.kt:131-216`):
- Toolbar row: `BrowseMangaSourceToolbar` (P1§3.2).
- Chip row (horizontal scroll, `horizontalScroll(rememberScrollState())`, padding `small`, spacing `small`):
  1. `Popular` (icon `Favorite`, label `popular`). Selected when `listing == Popular`. Click = `resetFilters()` then `setListing(Popular)` (B§4.6).
  2. `Latest` (icon `NewReleases`, label `latest`). Only if `supportsLatest`. Same pattern.
  3. `Filter` (icon `FilterList`, label `action_filter`). Only if `filters.isNotEmpty()`. Selected when `listing is Search`. Click = open sheet (`openFilterSheet`).
- Divider below the chips.

Content states (paging; B§5):

| Condition | Visible |
|---|---|
| first page loading, no items | spinner (P1§3.3) |
| page 1 failed (`ErrorFirst`) | error empty state with actions (P1§3.3) |
| page 1 empty | `NoChaptersException` -> `no_results_found` ("No results found") in the same empty state (B§4.3) |
| items present, next page loading | spinner row at the end (P1§3.3) |
| next page failed (`ErrorAppend`) | error bar at the bottom with retry (P1§3.3) |
| end of list | no spinner |
| stub source | `MissingSourceScreen` only (`BrowseMangaSourceScreen.kt:101-107`) |

Navigate-up (B§4.6.2): with an active search and toolbar text that is not the listing query, back clears the toolbar text first. Otherwise back pops.

Long press on a row (B§4.8.4): decision table (section 4).

Click on a row: `MangaScreen(manga.id, true)` (`BrowseMangaSourceScreen.kt:231`).

### 2.5 S-E Filter sheet (B§6)

Container: `AdaptiveSheet` (P1 reference). Sticky header row: `Reset` (text button, primary colour) on the start; `Filter` (filled button) on the end; divider below. Body: the filter list (B§6, table).

Per-type widgets and their GTK mapping:

| Filter type | Compose widget | GTK 4 widget | Notes |
|---|---|---|---|
| `Header` | `HeadingItem(name)` | `gtk::Label` heading style (`.heading`) | not interactive |
| `Separator` | `HorizontalDivider()` | `gtk::Separator` (horizontal) | |
| `CheckBox` | `CheckboxItem(label, checked)` | `gtk::CheckButton` in a row | click toggles |
| `TriState` | `TriStateItem(label, state)` | row with a `gtk::Label` state text ("Off" / "Include" / "Exclude") + the row click | click cycles Disabled -> EnabledIs -> EnabledNot -> Disabled (B§6.2) |
| `Text` | `TextItem(label, value)` | `gtk::Entry` with placeholder = name | every keystroke updates state (no search) |
| `Select` | `SelectItem(label, options, selected)` | `gtk::DropDown` with `gtk::StringList` | selected text = `values[state]` (clamped, D-BR-01 variant) |
| `Sort` | `CollapsibleBox(heading)` + `SortItem` rows | `gtk::Expander` with one row per value; arrow icon `go-up-symbolic` / `go-down-symbolic` on the selected row | click on the selected row flips direction; click on another row keeps the direction (B§6.1) |
| `Group` | `CollapsibleBox(heading)` + children | `gtk::Expander` with children rendered recursively | the group's children are the same widget set |

Exact interaction rules (code-verified, B§6):
- Every control change calls `onUpdate` -> `setFilters` (state only). Nothing searches until Filter is pressed.
- Filter button: runs the search with the current filter values, then closes the sheet (`SourceFilterMangaDialog.kt:62-66`).
- Reset button: replaces the state filters with the source defaults; the sheet stays open; no search (B§4.10).
- Sort with no selection: the Kotlin crashes on first click (`filter.state!!`). Kurobayomi: select the clicked item ascending (D-BR-01).
- A `TriState` raw value other than 0..2 renders as "Off" with a log line (D-BR-02). Kotlin crashes.

### 2.6 S-F Global search (state = `MangaSearchScreenModel.State`, B§9)

Wrapper behavior (B§7):

| Condition | Visible |
|---|---|
| sources not initialised (`ifMangaSourcesLoaded` false) | `LoadingScreen` |
| entry with a query and an extension filter, exactly one selected source (`state.total == 1`) | `LoadingScreen` until that source's result arrives |
| that single source returns exactly one manga | the screen is replaced by the manga page `MangaScreen(id, true)` (no back entry for the search) |
| that single source returns 0 or several manga, or errors | normal results screen |
| normal | toolbar + chips + results (P1§10) |

Results (B§9.5, B§9.6):
- Rows in the order of `filteredItems`: non-empty results first, then pinned sources, then name. Rows reorder as results arrive.
- `onlyShowHasResults` on: only non-empty results are rendered; loading and error rows disappear (B§9.1 `isVisible`).
- Progress bar: visible while `1 <= progress < total`; value `progress / total` (P1§10.1).

Chips and controls:
- `Pinned` chip (selected when `sourceFilter == PinnedOnly`): `setSourceFilter(PinnedOnly)`. This calls `search()` (B§9.7; `MangaSearchScreenModel.kt:115-118`). Consequence: changing the chip reruns the query for the sources that were not already searched with the same query.
- `All` chip: `setSourceFilter(All)` (same behavior).
- `Has results` chip: `toggleFilterResults()`, persisted in `__APP_STATE_has_filters_toggle_state` (B§9.7). Shared with migrate search.
- Search entry: `updateSearchQuery` on change (state only). `search()` on submit (B§9.3). No debounce.
- Header row click (source): opens `BrowseMangaSourceScreen(source.id, state.searchQuery)` (`GlobalMangaSearchScreen.kt:74-76`). Uses the text currently in the field, submitted or not.
- Card click or long press: `MangaScreen(id, true)` (`GlobalMangaSearchScreen.kt:77-78`). No long-press action exists.

Error rows: raw error message, or `unknown_error` when the message is null (P1§10.2). There is no retry control; a new search with a different query or a different source filter is the only way to re-request a failed source (B§9.3).

### 2.7 S-G Migrate source picker (state = `MigrateMangaSourceScreenModel.State`, B§11)

| State | Visible |
|---|---|
| `isLoading` | `LoadingScreen` |
| `isEmpty` (no source has favorites) | `EmptyScreen` `information_empty_library` ("Your library is empty") (P1§11) |
| list | sticky header + rows (P1§11) |

Sticky header controls:
- Sort mode button: alphabetical (`SortByAlpha`) or total (`Numbers`). Click toggles mode and writes both prefs (B§11.4).
- Direction button: ascending (`ArrowUpward`) or descending (`ArrowDownward`). Click toggles direction.
- Help button (tab action): opens `https://aniyomi.org/help/guides/source-migration/` (B§11.5).

Failure: the failure event is not shown in Kotlin (B§11.3). Kurobayomi shows the `internal_error` toast (D-MG-02).

### 2.8 S-H Duplicate dialog (add to library, B§4.8.4)

Opened when a non-favorite row has a favorite with the same title (lowercase compare) in any source.
- Confirm: add the manga to library anyway (`addFavorite`). Button text: the presentation's confirm label (`DuplicateMangaDialog`, not in part-2 scope).
- Open: `MangaScreen(duplicate.id)`.
- Migrate: opens S-K with `oldManga = duplicate`, `newManga = manga`.
- Dismiss: closes.

### 2.9 S-I Remove confirm (B§4.8.4)

Opened by long press on a favorite row. Title `are_you_sure` ("Are you sure?"), text `remove_manga` ("You are about to remove "%s" from your library" with the manga title). Confirm = `changeMangaFavorite` (remove). Cancel = dismiss. No undo.

### 2.10 S-J Category chooser (B§4.8.2)

Opened only in the "Always ask" path (default category `-1`, user categories exist). Checkboxes = user categories (system category excluded), preselected = current category links of the manga. Confirm = favorite the manga, then set links to the checked ids (the exclude output is ignored). "Edit categories" = push the Categories tab with the manga category section shown (`CategoriesTab.showMangaCategory()`).

### 2.11 Toasts and snackbars

| Event | Text key | Where |
|---|---|---|
| Sources list failure | `internal_error` | S-A |
| Sources filter failure | `internal_error` (then pop) | S-C |
| Migrate picker failure | `internal_error` (Kurobayomi; not in Kotlin) | S-G (D-MG-02) |
| Favorite write failure | `internal_error` (Kurobayomi; D-FAV-02) | S-D |

Kotlin shows these through `context.toast` (sources filter) or a snackbar (sources list). Kurobayomi: one in-window toast style (P1 overlay), 3 s.

---

## 3. Dialog and action decision table (S-D long press)

| Row state | Duplicate title exists | Action |
|---|---|---|
| favorite | any | S-I remove confirm |
| not favorite | yes | S-H duplicate dialog |
| not favorite | no | add to library via the default category rule (B§4.8.2): default `-1` with user categories -> S-J; default `0` or no user categories -> add with no category; default id of an existing category -> add with that category |

---

## 4. Keyboard and mouse (desktop mapping for these models)

| Action | Keyboard | Mouse |
|---|---|---|
| Open source (Popular) | Enter on the row | click |
| Latest | `L` on the row (Kurobayomi addition, no Kotlin equivalent) | click Latest button |
| Pin | `P` on the row (addition) | click pin icon |
| Options | `Menu` key or Shift+F10 on the row | long press / right click |
| Browse search | `Ctrl+F` opens the search entry; Enter submits; Escape clears and closes (B§4.6.2) | click search icon |
| Filter sheet | `Ctrl+Shift+F` or the chip; Enter applies (Filter button); Escape closes | click |
| Global search submit | Enter in the search entry | click search icon |
| Global search chips | Tab order | click |
| Migrate sort toggles | Tab to button, Space | click |

Additions (`L`, `P`, `Ctrl+F`, `Ctrl+Shift+F`) are Kurobayomi conveniences; they do not change model behavior.

---

## 5. Responsive behaviour

- Browse grid columns: from `portrait` or `landscape` column preference (area 37); `0` = adaptive with minimum 128 logical px (B§4.5). The grid uses the window's current orientation: landscape when width >= height (P1§3.4 proposal).
- Filter sheet: fixed width 480 px, or the window width minus 32 px when narrower; height up to 80% of the window (Kurobayomi sizing, not Kotlin).
- Chip rows scroll horizontally when they overflow (`gtk::ScrolledWindow` with `policy = (Automatic, Never)`).
- Dialogs use `gtk::Window` transient to the main window, modal, with the Kotlin text sizes.

---

## 6. Animations

Kotlin uses `Modifier.animateItem()` on source rows, global search rows and migrate rows (B§2.2 step, P1§3.1). Kurobayomi: no item animation (GTK `ListView` default). Sheets: GTK default transition. No Kotlin animation is a requirement.

---

## 7. Mockup gap list (current `/home/blackbox/kurobayomi/src/entry.rs`)

| Screen | Mockup now | Gap for part 2 |
|---|---|---|
| S-A Sources | `sources()` (line 283) with rows "Source A", "Source B" | no grouping, pins, last-used, options dialog, Latest button, data from registry |
| S-B Options | none | build dialog with pin / disable rows |
| S-C Sources filter | none (only row list in `sources`) | language switches, per-source checkboxes, error toast path |
| S-D Browse source | `browse_source(id, migrating)` (line 380), static cards | paging with 25-item prefetch, error and empty states, listing chips, filter sheet, navigate-up rule, long-press flow |
| S-E Filter sheet | none | all filter widget types, reset/filter semantics |
| S-F Global search | `global_search(id, migrating)` (line 432), static | pinned/all chips, has-results toggle, per-source states, single-result auto-open, sort order |
| S-G Migrate picker | `migrate_sources(anime)` (line 360), rows with counts 12/4/1 | real favorite counts, sort mode/direction buttons, help button |
| S-H/I/J/K dialogs | none | duplicate, remove, category chooser, migrate hand-off |

---

## 8. State matrix (summary)

| Screen | Loading | Empty | Error | Items | Dialog |
|---|---|---|---|---|---|
| S-A | `isLoading` | `source_empty_screen` | toast `internal_error` | rows | S-B |
| S-C | `Loading` | `source_filter_empty_screen` | toast + pop | rows | none |
| S-D | spinner | `no_results_found` | error empty state / error bar | grid or list | S-E, S-H, S-I, S-J, S-K |
| S-E | n/a | n/a | n/a | sections | n/a |
| S-F | `LoadingScreen` (wrapper) or per-source spinner | `no_results_found` per source (empty success) | raw message per source | cards per source | none |
| S-G | `LoadingScreen` | `information_empty_library` | toast `internal_error` (Kurobayomi) | rows | none |

---

## 9. Conflicts with part 1 and corrections

1. Global search chips rerun the query. Part 1 (`p1-ui-parity.md` §10.1) says changing the chip "only applies when triggering a new search" and that the query is not rerun. The code does rerun: `setSourceFilter` calls `search()` (`MangaSearchScreenModel.kt:115-118`). `search()` returns early only when the query is the same AND the filter is the same. A chip change therefore fetches the newly selected sources. Kurobayomi follows the code. Part 1 should be corrected (task T-40b-062 depends on this decision).
2. Migrate search origin-first ordering (part 1 §10.4 and migrate sections): the base model's sort overrides the subclass order (B§10). Kurobayomi implements the documented intent (origin first) (D-MG-01).
3. Browse "Popular" chip: part 1 §3.1 lists Latest and Popular as chips; Kotlin shows the Latest chip only for `supportsLatest` (`BrowseMangaSourceScreen.kt:175`). Part 1 agrees.
4. Sources page first paint: part 1 does not specify the pre-init empty state. Kurobayomi keeps the spinner until registry init (D-SS-01).

---

## 10. Strings used by these screens

See `p2-behavior.md` section 16 for the full English table. Keys owned by this file: `internal_error`, `source_empty_screen`, `source_filter_empty_screen`, `information_empty_library`, `no_results_found`, `unknown_error`, `popular`, `latest`, `action_filter`, `action_reset`, `action_pin`, `action_unpin`, `action_disable`, `are_you_sure`, `remove_manga`, `migration_help_guide`, `label_migration`, `label_manga_sources`, `action_global_search`, `has_results`, `default_category_summary`.
