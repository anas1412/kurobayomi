//! The filters of one browse page: the source's defaults, the sheet that edits a working copy, and the list a search
//! starts from (T-40b-031, T-40b-036). The defaults are never edited: every reset and every Popular or Latest search takes
//! a fresh clone, and nothing compares filter trees for identity. Pure; the sheet widget is in `filter_sheet`.

use std::cell::RefCell;

use kuro_source::filter_list::FilterList;
use kuro_source::filter_sheet::FilterSheet;

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
    fn reset_replaces_every_filter_with_a_fresh_one() {
        let filters = SourceFilters::new(defaults());
        filters.edit(|sheet| sheet.toggle_check(&[0]).expect("check"));
        filters.reset();
        assert_eq!(filters.current(), defaults());
        assert_eq!(filters.items().len(), 2);
    }
}
