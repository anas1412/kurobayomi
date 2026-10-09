//! Favorite routing of the manga page (T-38b-011). Upstream: `MangaScreenModel.kt` `toggleFavorite` (behaviour 4.2
//! step 5) and `GetDuplicateLibraryManga`. Pure: the page reads the categories and runs the chosen branch. The
//! duplicate check and the remove branch run before this.

use kuro_data::category::Category;

/// The add branch that runs when the manga is not a favorite and has no duplicate (behaviour 4.2 step 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddRoute {
    /// The default category exists: favorite and that one category, in one transaction.
    /// `None` means no category: the default is `0` ("Uncategorized"), or there are no user categories.
    AddDefaultCategory(Option<i64>),
    /// The default is unset or was deleted: the category dialog opens, and favorite is not written yet (E-FAV-3).
    ChooseCategory,
}

/// The default category id is `-1` when unset, `0` for "Uncategorized", otherwise a user category id.
pub fn route_add(default_category: i64, user_categories: &[Category]) -> AddRoute {
    if let Some(category) = user_categories.iter().find(|c| c.id == default_category) {
        return AddRoute::AddDefaultCategory(Some(category.id));
    }
    if default_category == 0 || user_categories.is_empty() {
        return AddRoute::AddDefaultCategory(None);
    }
    AddRoute::ChooseCategory
}

/// The duplicate check before a favorite write (E-FAV-1, D-9): the first other favorite with the same title, in any
/// source. Titles compare with Unicode lowercasing, because SQLite's `LOWER` only folds ASCII. `library` holds
/// `(id, title)` of the favorites; the entry itself never counts as its own duplicate.
pub fn find_duplicate(id: i64, title: &str, library: &[(i64, &str)]) -> Option<i64> {
    let wanted = title.to_lowercase();
    library
        .iter()
        .find(|(other, other_title)| *other != id && other_title.to_lowercase() == wanted)
        .map(|(other, _)| *other)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn category(id: i64) -> Category {
        Category { id, name: format!("c{id}"), order: id, flags: 0, hidden: false }
    }

    #[test]
    fn route_default_category_exists() {
        assert_eq!(route_add(3, &[category(2), category(3)]), AddRoute::AddDefaultCategory(Some(3)));
    }

    #[test]
    fn route_default_uncategorized() {
        assert_eq!(route_add(0, &[category(2)]), AddRoute::AddDefaultCategory(None));
    }

    #[test]
    fn route_no_categories() {
        assert_eq!(route_add(-1, &[]), AddRoute::AddDefaultCategory(None));
        assert_eq!(route_add(5, &[]), AddRoute::AddDefaultCategory(None));
    }

    #[test]
    fn route_default_deleted_choose() {
        assert_eq!(route_add(9, &[category(2)]), AddRoute::ChooseCategory);
    }

    #[test]
    fn route_default_unset_choose() {
        assert_eq!(route_add(-1, &[category(2)]), AddRoute::ChooseCategory);
    }

    #[test]
    fn duplicate_check_unicode_case() {
        let library = [(1, "Ökonomie"), (2, "École"), (3, "Other")];
        assert_eq!(find_duplicate(9, "ÉCOLE", &library), Some(2), "Unicode case folds, not only ASCII");
        assert_eq!(find_duplicate(9, "ökonomie", &library), Some(1));
        assert_eq!(find_duplicate(2, "École", &library), None, "an entry is never its own duplicate");
        assert_eq!(find_duplicate(9, "Ecole", &library), None, "accents still differ");
        assert_eq!(find_duplicate(9, "Missing", &library), None);
    }

    #[test]
    fn duplicate_check_returns_the_first_match() {
        let library = [(4, "Same"), (5, "same")];
        assert_eq!(find_duplicate(9, "SAME", &library), Some(4));
    }
}
