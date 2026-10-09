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
}
