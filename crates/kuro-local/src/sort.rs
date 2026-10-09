//! Browse sort: the `Order by` filter (Title or Date) and `sort_entries` (area:14 section 6, T-14a-013).

use std::cmp::Ordering;

use kuro_source::{Filter, SortSelection};

use crate::fs::FsEntry;

/// `MangaOrderBy.Popular` / `AnimeOrderBy.Popular`: Title ascending. The name is misleading: there is no popularity.
pub const POPULAR_SORT: SortSelection = SortSelection {
    index: 0,
    ascending: true,
};
/// `MangaOrderBy.Latest` / `AnimeOrderBy.Latest`: Date descending.
pub const LATEST_SORT: SortSelection = SortSelection {
    index: 1,
    ascending: false,
};

/// The one Sort filter of the browse screen, labelled `Order by` with the options `Title` and `Date`.
pub fn order_by(state: Option<SortSelection>) -> Filter {
    Filter::Sort {
        name: "Order by".to_owned(),
        values: vec!["Title".to_owned(), "Date".to_owned()],
        state,
    }
}

/// The selection of the Sort filter in a filter list, or `None` when the list has no Sort filter or it has no state.
pub fn sort_from(filters: &[Filter]) -> Option<SortSelection> {
    filters
        .iter()
        .find_map(|f| match f {
            Filter::Sort { state, .. } => Some(*state),
            _ => None,
        })
        .flatten()
}

/// Sorts folder entries the way the browse screen does. Index 0 is by name (case-insensitive, not natural: `10`
/// comes before `9`), index 1 by modification time. Any other index keeps the order it was given.
pub fn sort_entries(entries: &mut [FsEntry], sort: SortSelection) {
    let asc = sort.ascending;
    match sort.index {
        0 => entries.sort_by(|a, b| directed(asc, ci_cmp(&a.name, &b.name))),
        1 => entries.sort_by(|a, b| directed(asc, a.modified_ms.cmp(&b.modified_ms))),
        _ => {}
    }
}

fn directed(ascending: bool, ordering: Ordering) -> Ordering {
    if ascending { ordering } else { ordering.reverse() }
}

/// Java `String.CASE_INSENSITIVE_ORDER`: each char goes through uppercase then lowercase, a char whose mapping is
/// more than one char stays as it is.
pub fn ci_cmp(a: &str, b: &str) -> Ordering {
    a.chars().map(fold).cmp(b.chars().map(fold))
}

fn fold(c: char) -> char {
    let upper = single(c.to_uppercase(), c);
    single(upper.to_lowercase(), upper)
}

fn single(mut mapped: impl Iterator<Item = char>, fallback: char) -> char {
    match (mapped.next(), mapped.next()) {
        (Some(only), None) => only,
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn entry(name: &str, modified_ms: i64) -> FsEntry {
        FsEntry {
            name: name.to_owned(),
            path: PathBuf::from(name),
            is_dir: true,
            is_file: false,
            modified_ms,
        }
    }

    fn names(entries: &[FsEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn the_defaults_are_title_ascending_and_date_descending() {
        assert_eq!(
            POPULAR_SORT,
            SortSelection {
                index: 0,
                ascending: true
            }
        );
        assert_eq!(
            LATEST_SORT,
            SortSelection {
                index: 1,
                ascending: false
            }
        );
    }

    #[test]
    fn the_order_by_filter_has_the_two_labels() {
        let filter = order_by(Some(POPULAR_SORT));
        let Filter::Sort { name, values, state } = filter else {
            panic!("not a sort filter")
        };
        assert_eq!(name, "Order by");
        assert_eq!(values, ["Title", "Date"]);
        assert_eq!(state, Some(POPULAR_SORT));
    }

    #[test]
    fn title_sort_is_not_natural() {
        let mut list = vec![entry("9", 0), entry("10", 0), entry("a", 0)];
        sort_entries(&mut list, POPULAR_SORT);
        assert_eq!(names(&list), ["10", "9", "a"]);
    }

    #[test]
    fn title_sort_ignores_case_and_descending_reverses() {
        let mut list = vec![entry("b", 0), entry("A", 0), entry("c", 0)];
        sort_entries(
            &mut list,
            SortSelection {
                index: 0,
                ascending: true,
            },
        );
        assert_eq!(names(&list), ["A", "b", "c"]);
        sort_entries(
            &mut list,
            SortSelection {
                index: 0,
                ascending: false,
            },
        );
        assert_eq!(names(&list), ["c", "b", "A"]);
    }

    #[test]
    fn date_sort_orders_by_modification_time() {
        let mut list = vec![entry("old", 1), entry("new", 3), entry("mid", 2)];
        sort_entries(&mut list, LATEST_SORT);
        assert_eq!(names(&list), ["new", "mid", "old"]);
        sort_entries(
            &mut list,
            SortSelection {
                index: 1,
                ascending: true,
            },
        );
        assert_eq!(names(&list), ["old", "mid", "new"]);
    }

    #[test]
    fn the_sort_selection_comes_from_the_sort_filter() {
        assert_eq!(sort_from(&[order_by(Some(LATEST_SORT))]), Some(LATEST_SORT));
        assert_eq!(sort_from(&[Filter::text("q"), order_by(None)]), None);
        assert_eq!(sort_from(&[Filter::text("q")]), None);
        assert_eq!(sort_from(&[]), None);
    }

    #[test]
    fn an_unknown_index_keeps_the_order() {
        let mut list = vec![entry("b", 0), entry("a", 0)];
        sort_entries(
            &mut list,
            SortSelection {
                index: 7,
                ascending: true,
            },
        );
        assert_eq!(names(&list), ["b", "a"]);
    }
}
