//! `FilterList`: the filters a source offers, in order (`FilterList.kt`, T-15a-011).

use std::ops::Deref;

use crate::filter::Filter;

/// The filters of one source. Equality is structural. Kotlin's `FilterList.equals` is always false; the
/// Rust type deliberately differs (EC-FI-11).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct FilterList(pub Vec<Filter>);

impl Deref for FilterList {
    type Target = [Filter];

    fn deref(&self) -> &[Filter] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_list_is_empty() {
        let list = FilterList::default();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn it_derefs_to_a_slice_and_clones_independently() {
        let list = FilterList(vec![Filter::text("Query")]);
        assert_eq!(list[0].name(), "Query");
        let mut copy = list.clone();
        copy.0.push(Filter::check_box("Flag"));
        assert_eq!(list.len(), 1);
        assert_eq!(copy.len(), 2);
    }

    #[test]
    fn a_list_is_shared_as_an_arc_without_copying() {
        let shared = std::sync::Arc::new(FilterList(vec![Filter::text("Query")]));
        let other = std::sync::Arc::clone(&shared);
        assert!(std::sync::Arc::ptr_eq(&shared, &other));
        assert_eq!(other[0].name(), "Query");
    }

    #[test]
    fn equality_is_structural() {
        let a = FilterList(vec![Filter::check_box("Flag")]);
        assert_eq!(a, a.clone());
        assert_ne!(a, FilterList(vec![Filter::check_box("Other")]));
        assert_eq!(FilterList::default(), FilterList(Vec::new()));
    }
}
