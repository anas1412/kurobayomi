//! Dialog routing: one dialog slot, typed dialog kinds, and the change-category dialog state.
//! Upstream: `MangaScreenModel.kt:1094-1106` (Dialog), `:382-396` (showChangeCategoryDialog), `:455-462` (confirm).

/// A category row in the change-category dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryItem {
    pub id: i64,
    pub name: String,
    pub checked: bool,
}

/// The change-category dialog: a snapshot of the user categories with the manga's current membership.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryDialog {
    pub manga_id: i64,
    /// True in the add-to-library flow, false when the manga is already a favorite.
    pub add_flow: bool,
    pub items: Vec<CategoryItem>,
}

impl CategoryDialog {
    /// Lists user categories only (`id != 0`). `membership` may be empty, which gives all rows unchecked
    /// (E-CAT-1: an empty membership is not the default category).
    pub fn new(
        manga_id: i64,
        add_flow: bool,
        categories: &[(i64, String)],
        membership: &[i64],
    ) -> Self {
        let items = categories
            .iter()
            .filter(|(id, _)| *id != 0)
            .map(|(id, name)| CategoryItem {
                id: *id,
                name: name.clone(),
                checked: membership.contains(id),
            })
            .collect();
        Self {
            manga_id,
            add_flow,
            items,
        }
    }

    /// A plain picker over `(id, name, checked)` rows, for a screen outside the entry page (Browse, History).
    pub fn from_rows(rows: Vec<(i64, String, bool)>) -> Self {
        let items = rows.into_iter().map(|(id, name, checked)| CategoryItem { id, name, checked }).collect();
        Self { manga_id: 0, add_flow: false, items }
    }

    /// Flips the checkbox at `index`. Out-of-range indexes do nothing.
    pub fn toggle(&mut self, index: usize) {
        if let Some(item) = self.items.get_mut(index) {
            item.checked = !item.checked;
        }
    }

    /// The checked category ids only, in list order.
    pub fn checked_ids(&self) -> Vec<i64> {
        self.items
            .iter()
            .filter(|i| i.checked)
            .map(|i| i.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cats() -> Vec<(i64, String)> {
        vec![
            (0, "Default".into()),
            (2, "Reading".into()),
            (3, "Later".into()),
        ]
    }

    #[test]
    fn dialog_lists_user_categories_only_with_membership() {
        let d = CategoryDialog::new(7, false, &cats(), &[3]);
        assert_eq!(d.items.len(), 2);
        assert!(!d.items[0].checked);
        assert!(d.items[1].checked);
    }

    #[test]
    fn empty_membership_is_empty_selection() {
        let d = CategoryDialog::new(7, true, &cats(), &[]);
        assert!(d.checked_ids().is_empty());
    }
}
