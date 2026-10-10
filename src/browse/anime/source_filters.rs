//! The source-filter rules of the anime browse sheet (T-41b-029, T-41b-030, T-41b-050; `p2-topic-source-filters.md` sections
//! 3 and 7). The state types and the sort tap belong to the source layer (`kuro-source`), and the genre walk is
//! `filters::genre_search`. This module gives the spec's names to those rules, with the spec's test vectors. It keeps no
//! second copy of the rules.

use kuro_source::filter::{Filter, SortSelection, TriStateState};
use kuro_source::filter_list::FilterList;

use crate::browse::filters::genre_search;

/// `TriState.next()`: Ignore, then Include, then Exclude, then Ignore again.
pub fn cycle_tristate(state: TriStateState) -> TriStateState {
    state.next()
}

/// The state of a raw value. An unknown value is Ignore and the flag is set so the caller can log it (D-4, E-BR-28).
pub fn tristate_from_int(raw: i64) -> (TriStateState, bool) {
    match i32::try_from(raw).ok().and_then(|raw| TriStateState::from_raw(raw).ok()) {
        Some(state) => (state, false),
        None => (TriStateState::Ignore, true),
    }
}

/// A tap on the sort row `index`: the same row flips the direction, another row keeps it, and no selection is ascending.
pub fn select_sort(current: Option<SortSelection>, index: i32) -> SortSelection {
    SortSelection::tap(current, index)
}

/// `Some(true)` when the row shows the descending arrow, `None` when it shows no arrow.
pub fn sort_arrow(current: Option<SortSelection>, index: i32) -> Option<bool> {
    current.filter(|selection| selection.index == index).map(|selection| !selection.ascending)
}

/// The name of any filter variant. Headers and separators have names too.
pub fn filter_name(filter: &Filter) -> &str {
    filter.name()
}

/// Whether a genre matched a filter of the source's defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenreOutcome {
    Matched,
    NotMatched,
}

/// `apply_genre`: the filters a genre search runs with, on a fresh copy of `defaults`, and whether one matched. A match
/// searches with no text; no match searches the genre name as text (B§4.7).
pub fn apply_genre(defaults: &FilterList, genre: &str) -> (GenreOutcome, FilterList) {
    let (query, filters) = genre_search(defaults, genre);
    let outcome = if query.is_none() { GenreOutcome::Matched } else { GenreOutcome::NotMatched };
    (outcome, filters)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(index: i32, ascending: bool) -> Option<SortSelection> {
        Some(SortSelection { index, ascending })
    }

    fn defaults() -> FilterList {
        FilterList(vec![
            Filter::check_box("Flag"),
            Filter::group("Genres", vec![Filter::tri_state("Action"), Filter::check_box("Drama"), Filter::text("Note")]),
        ])
    }

    #[test]
    fn the_tristate_cycle_and_the_raw_mapping_follow_the_vectors() {
        assert_eq!(cycle_tristate(TriStateState::Ignore), TriStateState::Include);
        assert_eq!(cycle_tristate(TriStateState::Include), TriStateState::Exclude);
        assert_eq!(cycle_tristate(TriStateState::Exclude), TriStateState::Ignore);
        assert_eq!(tristate_from_int(3), (TriStateState::Ignore, true), "D-4: an unknown value is Ignore and is reported");
        assert_eq!(tristate_from_int(1), (TriStateState::Include, false));
        assert_eq!(tristate_from_int(-1), (TriStateState::Ignore, true));
    }

    #[test]
    fn the_sort_selection_and_arrow_follow_the_vectors() {
        assert_eq!(select_sort(None, 2), SortSelection { index: 2, ascending: true }, "no selection is ascending");
        assert_eq!(select_sort(sel(1, true), 1), SortSelection { index: 1, ascending: false }, "the same row flips");
        assert_eq!(select_sort(sel(1, true), 3), SortSelection { index: 3, ascending: true }, "another row keeps the direction");
        assert_eq!(sort_arrow(sel(1, false), 1), Some(true));
        assert_eq!(sort_arrow(sel(1, false), 2), None);
        assert_eq!(sort_arrow(None, 0), None);
    }

    #[test]
    fn the_filter_name_is_the_name_of_every_variant() {
        assert_eq!(filter_name(&Filter::header("Sort by")), "Sort by");
        assert_eq!(filter_name(&Filter::separator("---")), "---");
        assert_eq!(filter_name(&Filter::select("Genre", vec!["Any".into()])), "Genre");
    }

    #[test]
    fn a_genre_matches_by_name_and_sets_its_filter_on_a_copy() {
        let base = defaults();
        let (outcome, filters) = apply_genre(&base, "ACTION");
        assert_eq!(outcome, GenreOutcome::Matched, "a group child TriState named Action matches");
        let Filter::Group { state, .. } = &filters.0[1] else { panic!("the group stays") };
        assert_eq!(state[0], Filter::TriState { name: "Action".into(), state: TriStateState::Include });
        assert_eq!(base, defaults(), "the defaults are not edited");
    }

    #[test]
    fn a_group_text_child_matches_with_no_state_change() {
        let (outcome, filters) = apply_genre(&defaults(), "note");
        assert_eq!(outcome, GenreOutcome::Matched, "E-BR-04: a Text child still counts as a match");
        assert_eq!(filters, defaults(), "and changes nothing");
    }

    #[test]
    fn a_root_checkbox_never_matches_and_a_missing_genre_is_text() {
        let root = FilterList(vec![Filter::check_box("Comedy")]);
        assert_eq!(apply_genre(&root, "comedy").0, GenreOutcome::NotMatched);
        assert_eq!(apply_genre(&defaults(), "zzz").0, GenreOutcome::NotMatched);
    }

    #[test]
    fn a_top_level_select_matches_its_option_and_sets_the_index() {
        let select = FilterList(vec![Filter::select("Genre", vec!["Any".into(), "Comedy".into()])]);
        let (outcome, filters) = apply_genre(&select, "comedy");
        assert_eq!(outcome, GenreOutcome::Matched);
        assert_eq!(filters.0[0].select_value(), Some("Comedy"), "state = 1");
    }

    #[test]
    fn two_identical_filter_lists_are_equal_and_an_edited_one_is_not() {
        let original = defaults();
        assert_eq!(original, defaults(), "E-FD-07: identical lists compare equal");
        let mut edited = defaults();
        edited.0[0] = Filter::CheckBox { name: "Flag".into(), state: true };
        assert_ne!(edited, original, "an edit makes the list unequal to the original");
    }
}
