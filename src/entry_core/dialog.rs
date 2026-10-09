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

/// What confirming the dialog writes. Both forms go in one transaction in the store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CategoryConfirm {
    AddToLibrary {
        manga_id: i64,
        category_ids: Vec<i64>,
    },
    SetCategories {
        manga_id: i64,
        category_ids: Vec<i64>,
    },
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

    pub fn confirm(&self) -> CategoryConfirm {
        let category_ids = self.checked_ids();
        if self.add_flow {
            CategoryConfirm::AddToLibrary {
                manga_id: self.manga_id,
                category_ids,
            }
        } else {
            CategoryConfirm::SetCategories {
                manga_id: self.manga_id,
                category_ids,
            }
        }
    }
}

/// Every dialog the details page can show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dialog {
    ChangeCategory(CategoryDialog),
    DeleteChapters(Vec<i64>),
    DuplicateManga { manga_id: i64, duplicate_id: i64 },
    Migrate { new_id: i64, old_id: i64 },
    SetFetchInterval(i64),
    SettingsSheet,
    TrackSheet,
    FullCover,
}

/// One dialog at a time. Opening a dialog replaces the current one (E-DIALOG-2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DialogSlot {
    current: Option<Dialog>,
    open_tracking_after: bool,
}

impl DialogSlot {
    pub fn open(&mut self, dialog: Dialog) {
        self.current = Some(dialog);
    }

    pub fn dismiss(&mut self) {
        self.current = None;
    }

    pub fn current(&self) -> Option<&Dialog> {
        self.current.as_ref()
    }

    /// Dismisses the change-category dialog without confirming. The add flow leaves the manga out of the library.
    /// The one-shot "open tracking" intent is set only when `auto_open_tracking` is on and it was the add flow.
    pub fn dismiss_category(&mut self, add_flow: bool, auto_open_tracking: bool) {
        self.current = None;
        if add_flow && auto_open_tracking {
            self.open_tracking_after = true;
        }
    }

    /// Consumes the one-shot tracking intent. It is true once, and then false.
    pub fn take_tracking_intent(&mut self) -> bool {
        std::mem::take(&mut self.open_tracking_after)
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

    #[test]
    fn confirm_writes_only_checked_in_the_right_form() {
        let mut d = CategoryDialog::new(7, true, &cats(), &[]);
        d.toggle(0);
        assert_eq!(
            d.confirm(),
            CategoryConfirm::AddToLibrary {
                manga_id: 7,
                category_ids: vec![2]
            }
        );
        d.toggle(0);
        d.toggle(1);
        d.add_flow = false;
        assert_eq!(
            d.confirm(),
            CategoryConfirm::SetCategories {
                manga_id: 7,
                category_ids: vec![3]
            }
        );
        d.toggle(9);
    }

    #[test]
    fn dismiss_add_flow_sets_tracking_intent_once() {
        let mut slot = DialogSlot::default();
        slot.open(Dialog::ChangeCategory(CategoryDialog::new(
            1,
            true,
            &cats(),
            &[],
        )));
        slot.dismiss_category(true, true);
        assert!(slot.current().is_none());
        assert!(slot.take_tracking_intent());
        assert!(
            !slot.take_tracking_intent(),
            "the intent is consumed exactly once"
        );
    }

    #[test]
    fn no_tracking_intent_without_the_setting_or_outside_add_flow() {
        let mut slot = DialogSlot::default();
        slot.dismiss_category(true, false);
        assert!(!slot.take_tracking_intent());
        slot.dismiss_category(false, true);
        assert!(!slot.take_tracking_intent());
    }

    #[test]
    fn opening_replaces_the_current_dialog() {
        let mut slot = DialogSlot::default();
        slot.open(Dialog::FullCover);
        slot.open(Dialog::SettingsSheet);
        assert_eq!(slot.current(), Some(&Dialog::SettingsSheet));
        slot.dismiss();
        assert!(slot.current().is_none());
    }
}
