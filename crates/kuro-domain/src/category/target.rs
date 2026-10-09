//! Add-to-library category resolution, shared by the library, browse, history, details and migration entry points
//! (behaviour 1.9, T-08a-030). Pure: the caller reads the default preference and the category ids.

/// Where a newly favourited entry goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddTarget {
    /// Put the entry in these categories (empty means the Default category).
    Categories(Vec<i64>),
    /// Ask the user which categories to use.
    Ask,
}

/// `default_*_category` is `-1` for "always ask", `0` for Default, or a category id. An id that no longer exists
/// asks.
pub fn resolve_add_target(default_pref: i64, category_ids: &[i64]) -> AddTarget {
    match default_pref {
        -1 if category_ids.is_empty() => AddTarget::Categories(Vec::new()),
        -1 => AddTarget::Ask,
        0 => AddTarget::Categories(Vec::new()),
        id if category_ids.contains(&id) => AddTarget::Categories(vec![id]),
        _ => AddTarget::Ask,
    }
}

/// Adds an entry: the favourite flag is written first and the categories second. A failed favourite write aborts
/// before any category is touched. `set_favorite` returns `Err` on failure.
pub fn add_to_library<E>(
    target: &AddTarget,
    set_favorite: impl FnOnce() -> Result<(), E>,
    set_categories: impl FnOnce(&[i64]),
) -> Result<(), E> {
    set_favorite()?;
    if let AddTarget::Categories(ids) = target {
        set_categories(ids);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn the_resolution_table_of_the_edge_cases() {
        let (a, b) = (1, 2);
        assert_eq!(resolve_add_target(-1, &[a, b]), AddTarget::Ask);
        assert_eq!(resolve_add_target(-1, &[]), AddTarget::Categories(vec![]));
        assert_eq!(resolve_add_target(0, &[a]), AddTarget::Categories(vec![]));
        assert_eq!(resolve_add_target(b, &[a, b]), AddTarget::Categories(vec![b]));
        assert_eq!(resolve_add_target(999, &[a, b]), AddTarget::Ask);
    }

    #[test]
    fn a_failed_favourite_write_aborts_before_the_categories() {
        let touched = Cell::new(false);
        let result = add_to_library(&AddTarget::Categories(vec![2]), || Err("db"), |_| touched.set(true));
        assert_eq!(result, Err("db"));
        assert!(!touched.get());
        let ok = add_to_library(
            &AddTarget::Categories(vec![2]),
            || Ok::<(), &str>(()),
            |ids| {
                assert_eq!(ids, &[2]);
                touched.set(true);
            },
        );
        assert_eq!(ok, Ok(()));
        assert!(touched.get());
    }
}
