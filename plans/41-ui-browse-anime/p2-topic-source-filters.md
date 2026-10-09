# Deep dive: source-defined filters (model, mutation rules, genre search)

Covers `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/SourceFilterAnimeDialog.kt` (sheet), the filter types in `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt` and `AnimeFilterList.kt`, and the genre-search path in `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:178-218`. Behavior summary: `p2-behavior.md` sections 5.15 and 6. Rust module: `browse/anime/source_filters.rs`.

## 1. Model

`FilterList` is a tree of `Filter` values (see `p2-rust-design.md` 3.2). Kotlin constants:
- TriState ints: `STATE_IGNORE = 0`, `STATE_INCLUDE = 1`, `STATE_EXCLUDE = 2` (`AnimeFilter.kt:20-24`).
- `Sort.Selection(index, ascending)` (`AnimeFilter.kt:31`).

Source defaults come from `AnimeSource.getFilterList()` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSource.kt:40`). Every call returns a new list with the source defaults. The browse model keeps one list as `State.filters` and mutates it in place from the sheet.

## 2. Rendering rules (the complete table)

| Variant | Visible control | Shows | Click or input | New state |
|---|---|---|---|---|
| `Header` | heading | `name` | none | none |
| `Separator` | divider | none | none | none |
| `CheckBox { state }` | checkbox row | `name`, checked = `state` | click | `!state` |
| `TriState { state }` | tri-state row | icon by state: ignored = `CheckBoxOutlineBlank`, include = `CheckBox`, exclude = `DisabledByDefault` | click | `cycle(state)` |
| `Text { state }` | text field | `name`, value = `state` | every edit | the new text |
| `Select { values, state }` | option list | options, selected index = `state` | choose option | index |
| `Sort { values, state }` | expandable list | one row per value; arrow on the selected value | click value | `select_sort(state, index)` |
| `Group { children }` | expandable list | `name`; each child rendered with these same rules | per child | per child |

Icons: `presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt:355-400` (`TriStateItem`). Tint is primary when the state is not ignored, and `onSurfaceVariant` when ignored.

The `TriStateItem` widget itself cycles the state (`DISABLED → ENABLED_IS → ENABLED_NOT → DISABLED`) and calls back with the next value. The sheet ignores that value and recomputes `next()` from the stored state. Both give the same result. Rust: call `cycle_tristate(state)` once, on the stored value.

## 3. Pure mutation functions (complete)

```rust
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriStateState { Ignore, Include, Exclude }

/// OBSERVED TriState.next(): DISABLED → ENABLED_IS → ENABLED_NOT → DISABLED, mapped to ints 0 → 1 → 2 → 0.
pub fn cycle_tristate(s: TriStateState) -> TriStateState {
    match s {
        TriStateState::Ignore => TriStateState::Include,
        TriStateState::Include => TriStateState::Exclude,
        TriStateState::Exclude => TriStateState::Ignore,
    }
}

/// OBSERVED SourceFilterAnimeDialog.toTriStateFilter(). Decision D-4: unknown ints are treated as Ignore
/// (Kotlin throws IllegalStateException). Returns the mapped state and a flag for logging.
pub fn tristate_from_int(v: i64) -> (TriStateState, bool) {
    match v {
        0 => (TriStateState::Ignore, false),
        1 => (TriStateState::Include, false),
        2 => (TriStateState::Exclude, false),
        _ => (TriStateState::Ignore, true),
    }
}

pub fn tristate_to_int(s: TriStateState) -> i64 {
    match s {
        TriStateState::Ignore => 0,
        TriStateState::Include => 1,
        TriStateState::Exclude => 2,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortSelection { pub index: usize, pub ascending: bool }

/// OBSERVED SourceFilterAnimeDialog.kt:137-141, with decision D-4 for the null case.
///  - same index: direction flips
///  - other index: direction kept
///  - no current selection (Kotlin: NullPointerException): index selected, ascending = true
pub fn select_sort(current: Option<SortSelection>, index: usize) -> SortSelection {
    match current {
        Some(c) if c.index == index => SortSelection { index, ascending: !c.ascending },
        Some(c) => SortSelection { index, ascending: c.ascending },
        None => SortSelection { index, ascending: true },
    }
}

/// OBSERVED SourceFilterAnimeDialog.kt:131-134.
/// Some(true) = show descending arrow (ascending == false on the selected index). None = no arrow.
pub fn sort_arrow(current: Option<SortSelection>, index: usize) -> Option<bool> {
    match current {
        Some(c) if c.index == index => Some(!c.ascending),
        _ => None,
    }
}

/// Name of any filter variant (Kotlin AnimeFilter.name). Headers and separators have names too.
pub fn filter_name(f: &Filter) -> &str {
    match f {
        Filter::Header { name } | Filter::Separator { name } | Filter::Select { name, .. }
        | Filter::Text { name, .. } | Filter::CheckBox { name, .. } | Filter::TriState { name, .. }
        | Filter::Group { name, .. } | Filter::Sort { name, .. } => name,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenreOutcome {
    /// A filter matched and was set (TriState → Include, CheckBox → true, Select → index).
    /// A Text, Header, Separator, Sort or Group child whose name matched still counts as a match with no state change
    /// (OBSERVED genreExists = true); see E-BR-04 for the consequence.
    Matched,
    /// No match. The caller uses the genre as a text query.
    NotMatched,
}

/// OBSERVED BrowseAnimeSourceScreenModel.searchGenre (lines 178-218).
/// Walks top-level filters in order. Group children are matched by name (case-insensitive).
/// Top-level Select is matched by option text. Other top-level variants are never matched.
pub fn apply_genre(filters: &mut FilterList, genre: &str) -> GenreOutcome {
    let wanted = genre.to_lowercase();
    for top in filters.0.iter_mut() {
        match top {
            Filter::Group { children, .. } => {
                for child in children.iter_mut() {
                    if filter_name(child).to_lowercase() == wanted {
                        match child {
                            Filter::TriState { state, .. } => *state = TriStateState::Include,
                            Filter::CheckBox { state, .. } => *state = true,
                            _ => {}
                        }
                        return GenreOutcome::Matched;
                    }
                }
            }
            Filter::Select { values, state, .. } => {
                if let Some(idx) = values.iter().position(|v| v.to_lowercase() == wanted) {
                    *state = idx;
                    return GenreOutcome::Matched;
                }
            }
            _ => {}
        }
    }
    GenreOutcome::NotMatched
}
```

Notes on `apply_genre`:
- The Kotlin version iterates the `state` of a Group, which holds its children. `filter_name(child)` covers every child type, as Kotlin does with `filter is AnimeSourceModelFilter<*>`.
- Kotlin `equals(genreName, true)` is case-insensitive comparison. Rust uses `to_lowercase()` on both sides. The two match for the scripts extensions use in practice.
- The Kotlin code treats a Group child match of any type as "exists". Rust returns `Matched` in the same cases, so the caller behaves the same (`Search(query = None)`). PROPOSAL (fixes E-BR-04): return `Matched` only for TriState and CheckBox children and for Select, and otherwise continue searching. This is a behavior change and needs a note in the release.

## 4. Genre search: caller contract

Rust caller (`BrowseModel::search_genre`):

```rust
pub fn search_genre(&mut self, genre: &str) {
    let mut defaults = self.source.filter_list();          // fresh source defaults (OBSERVED defaultFilters)
    let outcome = apply_genre(&mut defaults, genre);
    self.filters = defaults.clone();
    self.listing = match outcome {
        GenreOutcome::Matched => Listing::Search { query: None, filters: defaults },
        GenreOutcome::NotMatched => Listing::Search { query: Some(genre.to_owned()), filters: defaults },
    };
    self.toolbar_query = match &self.listing {
        Listing::Search { query, .. } => query.clone(),
        _ => None,
    };
    self.restart_pager();
}
```

Kotlin equivalent: `BrowseAnimeSourceScreenModel.kt:178-218`. `toolbarQuery = listing.query` (OBSERVED line 215).

## 5. Sheet state and actions

| Action | Effect | Listing changes? | Source |
|---|---|---|---|
| Reset | `filters = source.filter_list()` (new defaults) | no | `BrowseAnimeSourceScreenModel.kt:147-149` |
| Filter | `search(query = current, filters = self.filters)` then close | yes (new Search listing, pager restart) | `BrowseAnimeSourceScreen.kt:260` (`onFilter = { screenModel.search(filters = state.filters) }`) |
| Cancel / close | none; edits remain in `filters` | no | `SourceFilterAnimeDialog.kt:41-43` |
| Edit any control | writes the control's state in place | no | `SourceFilterAnimeDialog.kt:93-163` |

Behavior (E-FD-05): because the list is shared, edits made before Cancel are used by the next Filter press or the next text search.

Rust: `BrowseModel.filters` is the only list, and sheet edits are applied to it immediately, as in Kotlin, so Cancel does not revert edits. This is open question Q-5 in `p2-rust-design.md` section 12; the default is to keep the Kotlin behavior.

## 6. Equality and identity

- `AnimeFilter` equality: name and state (`AnimeFilter.kt:34-45`).
- `AnimeFilterList` equality: always false (`AnimeFilterList.kt:10-12`). Effect: `Listing.Search` never equals itself, so `distinctUntilChanged` in the browse listing stream does not suppress repeats (E-BR-06, E-BR-25).
- Rust: `FilterList` derives `PartialEq`. The browse listing compares by value (D-7). Filter edits made through the sheet change the value, so a Filter press always reloads.

## 7. Test vectors

| Input | Function | Expected |
|---|---|---|
| Ignore | `cycle_tristate` | Include |
| Include | `cycle_tristate` | Exclude |
| Exclude | `cycle_tristate` | Ignore |
| 3 | `tristate_from_int` | (Ignore, true) |
| `None`, index 2 | `select_sort` | index 2, ascending true |
| `Some{index 1, asc true}`, index 1 | `select_sort` | index 1, ascending false |
| `Some{index 1, asc true}`, index 3 | `select_sort` | index 3, ascending true |
| `Some{index 1, asc false}`, index 1 | `sort_arrow` | Some(true) |
| `Some{index 1, asc false}`, index 2 | `sort_arrow` | None |
| Group child TriState named "Action" | `apply_genre("action")` | Matched; child becomes Include |
| Group child CheckBox named "Drama" | `apply_genre("DRAMA")` | Matched; child becomes true |
| Top-level Select values ["Any", "Comedy"] | `apply_genre("comedy")` | Matched; state = 1 |
| Top-level CheckBox named "Comedy" | `apply_genre("comedy")` | NotMatched (root CheckBox never matched) |
| Group child Text named "Action" | `apply_genre("action")` | Matched with no state change (Kotlin behavior; E-BR-04) |
| No match anywhere | `apply_genre("zzz")` | NotMatched; caller uses text query |

## 8. Tests

- T-TEST-FILTER-TRISTATE: rows 1-3 of section 7.
- T-TEST-FILTER-SORT: rows 4-8.
- T-TEST-FILTER-GENRE: rows 9-14.
- T-TEST-FILTER-EQUALITY: two identical lists are equal; an edited list is not equal to the original (E-FD-07).
- Fixture: a `FakeSource` whose `filter_list()` contains Header, Separator, CheckBox, TriState, Text, Select, Sort (default `None`), Group (children: TriState, CheckBox, Text).
