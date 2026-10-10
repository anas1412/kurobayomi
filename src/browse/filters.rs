//! The filters of one browse page: the source's defaults, the sheet that edits a working copy, and the list a search
//! starts from (T-40b-031, T-40b-036). The defaults are never edited: every reset and every Popular or Latest search takes
//! a fresh clone, and nothing compares filter trees for identity. Pure; the sheet widget is in `filter_sheet`.

use std::cell::RefCell;

use kuro_source::browse::BrowseState;
use kuro_source::filter_list::FilterList;
use kuro_source::filter_sheet::FilterSheet;
use kuro_source::listing::Listing;

use super::nav::{FilterSeed, filter_seed};

/// `source_defaults` plus the sheet state of one browse page.
pub struct SourceFilters {
    defaults: FilterList,
    sheet: RefCell<FilterSheet>,
}

impl SourceFilters {
    pub fn new(defaults: FilterList) -> Self {
        let sheet = RefCell::new(FilterSheet::new(defaults.clone()));
        Self { defaults, sheet }
    }

    /// A fresh clone of the source's defaults.
    pub fn fresh(&self) -> FilterList {
        self.defaults.clone()
    }

    /// The source has filters to show, so the page offers the Filter chip.
    pub fn has_filters(&self) -> bool {
        !self.defaults.is_empty()
    }

    /// The sheet's current values, as a clone: a running search keeps its own copy of the list (RC-06).
    pub fn current(&self) -> FilterList {
        self.sheet.borrow().filters().clone()
    }

    /// Runs `edit` on the sheet. The widget calls this for every change.
    pub fn edit<R>(&self, edit: impl FnOnce(&mut FilterSheet) -> R) -> R {
        edit(&mut self.sheet.borrow_mut())
    }

    /// The sheet's rows now.
    pub fn items(&self) -> Vec<kuro_source::filter_sheet::SheetItem> {
        self.sheet.borrow().items()
    }

    /// The sheet takes `list` as its working copy (a genre search sets a filter): the sheet then shows it. It does not
    /// search.
    pub fn load(&self, list: FilterList) {
        self.sheet.borrow_mut().reset(list);
    }

    /// Reset: the sheet takes a fresh clone of the defaults. It does not search.
    pub fn reset(&self) {
        let fresh = self.fresh();
        self.sheet.borrow_mut().reset(fresh);
    }

    /// The filters a new search starts from. Starting from Popular or Latest takes the defaults, and the sheet follows
    /// them; a search that is already running keeps what the sheet holds (B§4.6.1).
    pub fn for_search(&self, running_search: bool) -> FilterList {
        match filter_seed(running_search) {
            FilterSeed::SourceDefaults => {
                self.reset();
                self.fresh()
            }
            FilterSeed::Keep => self.current(),
        }
    }
}

/// `searchGenre(genre)` of a browse page (T-40b-037, B§4.7): a group or select that holds the genre is set on a fresh copy of
/// the defaults, and the search runs with no text. Otherwise the genre name is searched as text with the defaults. Returns
/// the text (`None` for a genre filter) and the filters the search runs with.
pub fn genre_search(defaults: &FilterList, genre: &str) -> (Option<String>, FilterList) {
    let mut state = BrowseState::new(defaults.clone());
    state.search_genre(genre, defaults.clone());
    match state.listing() {
        Listing::Search { query, filters } => (query.clone(), filters.clone()),
        _ => (Some(genre.to_owned()), defaults.clone()),
    }
}

#[cfg(test)]
mod tests {
    use kuro_source::filter::Filter;

    use super::*;

    fn defaults() -> FilterList {
        FilterList(vec![Filter::check_box("Flag"), Filter::text("Author")])
    }

    #[test]
    fn fresh_is_an_independent_clone_of_the_defaults() {
        let filters = SourceFilters::new(defaults());
        let mut one = filters.fresh();
        one.0.push(Filter::check_box("Extra"));
        assert_eq!(filters.fresh().len(), 2, "the defaults are not edited through a clone");
        assert!(filters.has_filters());
        assert!(!SourceFilters::new(FilterList::default()).has_filters());
    }

    #[test]
    fn a_search_from_popular_or_latest_resets_the_sheet_to_the_defaults() {
        let filters = SourceFilters::new(defaults());
        filters.edit(|sheet| sheet.toggle_check(&[0]).expect("check"));
        assert_ne!(filters.current(), defaults(), "the sheet was edited");
        assert_eq!(filters.for_search(false), defaults());
        assert_eq!(filters.current(), defaults(), "the sheet follows the reset");
    }

    #[test]
    fn a_running_search_keeps_the_sheet_values_and_hands_out_a_copy() {
        let filters = SourceFilters::new(defaults());
        filters.edit(|sheet| sheet.set_text(&[1], "Oda".into()).expect("text"));
        let kept = filters.for_search(true);
        assert_eq!(kept, filters.current());
        filters.edit(|sheet| sheet.set_text(&[1], "later".into()).expect("text"));
        assert_ne!(kept, filters.current(), "an edit after the search does not reach the search's own copy");
    }

    #[test]
    fn a_loaded_list_becomes_the_sheets_working_copy_and_the_defaults_stay() {
        let filters = SourceFilters::new(defaults());
        let mut chosen = defaults();
        chosen.0[0] = Filter::check_box("Flag");
        chosen.0.push(Filter::check_box("Extra"));
        filters.load(chosen.clone());
        assert_eq!(filters.current(), chosen, "the sheet shows what a genre search set");
        assert_eq!(filters.fresh(), defaults(), "the defaults are never edited");
    }

    #[test]
    fn reset_replaces_every_filter_with_a_fresh_one() {
        let filters = SourceFilters::new(defaults());
        filters.edit(|sheet| sheet.toggle_check(&[0]).expect("check"));
        filters.reset();
        assert_eq!(filters.current(), defaults());
        assert_eq!(filters.items().len(), 2);
    }

    #[test]
    fn a_genre_in_a_group_checks_the_box_and_searches_with_no_text() {
        use kuro_source::filter::Filter;
        let defaults = FilterList(vec![Filter::group("Genres", vec![Filter::check_box("Action"), Filter::check_box("Drama")])]);
        let (query, filters) = genre_search(&defaults, "drama");
        assert_eq!(query, None, "B§4.7: a genre filter is searched with no text");
        let Filter::Group { state, .. } = &filters.0[0] else { panic!("the group stays") };
        assert!(matches!(state[1], Filter::CheckBox { state: true, .. }), "the matching checkbox is checked");
        assert!(matches!(state[0], Filter::CheckBox { state: false, .. }), "the other one is not");
        assert!(matches!(defaults.0[0], Filter::Group { ref state, .. } if matches!(state[1], Filter::CheckBox { state: false, .. })), "the defaults are never edited");
    }

    #[test]
    fn a_genre_in_a_select_sets_the_index_of_the_full_values_list() {
        use kuro_source::filter::Filter;
        let defaults = FilterList(vec![Filter::select("Genre", vec!["Any".into(), "Action".into(), "Drama".into()])]);
        let (query, filters) = genre_search(&defaults, "Drama");
        assert_eq!(query, None);
        assert_eq!(filters.0[0].select_value(), Some("Drama"), "B-EC-22: the index refers to the full values list");
    }

    #[test]
    fn a_genre_nobody_holds_is_searched_as_text_with_the_defaults() {
        use kuro_source::filter::Filter;
        let defaults = FilterList(vec![Filter::check_box("Flag")]);
        let (query, filters) = genre_search(&defaults, "Slice of Life");
        assert_eq!(query.as_deref(), Some("Slice of Life"), "B-EC-11: the genre name is the text");
        assert_eq!(filters, defaults);
    }
}
