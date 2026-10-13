//! `Category`, `CategoryUpdate` and the system ("Default") category rules (`Category.kt`).

use kuro_platform::kind::Kind;
use crate::SYSTEM_CATEGORY_ID;
use kuro_platform::sort::LibrarySort;

/// One row of `categories`. `order` is the `sort` column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub order: i64,
    pub flags: i64,
    pub hidden: bool,
}

impl Category {
    /// The Default category (id 0). Its stored name is empty; the UI shows the localized label instead.
    pub fn is_system(&self) -> bool {
        self.id == SYSTEM_CATEGORY_ID
    }

    /// The text to show: `default_label` (the localized "Default") for the system category, the stored name
    /// otherwise. A single helper so no screen repeats the rule.
    pub fn display_name<'a>(&'a self, default_label: &'a str) -> &'a str {
        if self.is_system() { default_label } else { &self.name }
    }

    /// The sort this category shows (`Category.sort(kind)`).
    pub fn sort(&self, kind: Kind) -> LibrarySort {
        LibrarySort::from_flags(kind, Some(self.flags))
    }
}

/// The sort of an optional category: `Category?.sort`, where a missing category gives the default sort.
pub fn sort_of(category: Option<&Category>, kind: Kind) -> LibrarySort {
    match category {
        Some(c) => c.sort(kind),
        None => LibrarySort::from_flags(kind, None),
    }
}

/// A partial update. `None` leaves the column unchanged (SQL `COALESCE`); all `None` is a legal no-op.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CategoryUpdate {
    pub id: i64,
    pub name: Option<String>,
    pub order: Option<i64>,
    pub flags: Option<i64>,
    pub hidden: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(id: i64, flags: i64) -> Category {
        Category { id, name: "A".into(), order: 0, flags, hidden: false }
    }

    #[test]
    fn system_category_is_id_zero_and_shows_the_label() {
        assert!(cat(0, 0).is_system());
        assert!(!cat(1, 0).is_system());
        assert_eq!(cat(0, 0).display_name("Default"), "Default");
        assert_eq!(cat(3, 0).display_name("Default"), "A");
    }

    #[test]
    fn sort_of_handles_the_null_case() {
        let asc_date = cat(2, 0x5C);
        assert_eq!(sort_of(Some(&asc_date), Kind::Manga).ty, kuro_platform::sort::SortType::DateAdded);
        assert_eq!(sort_of(None, Kind::Manga), LibrarySort::DEFAULT);
        assert_eq!(cat(2, 0).sort(Kind::Manga).dir, kuro_platform::sort::Direction::Descending);
    }

    #[test]
    fn update_default_is_a_no_op_update() {
        let u = CategoryUpdate::default();
        assert!(u.name.is_none() && u.order.is_none() && u.flags.is_none() && u.hidden.is_none());
    }
}
