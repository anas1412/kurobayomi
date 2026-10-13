//! The change-categories dialog model (behaviour B-13, T-37a-029). Pure: the view draws the rows and forwards clicks.
//! Rules come from kuro-domain's `category::bulk`, which already holds the tri-state order.

use kuro_domain::category::bulk::{self, Choice};
use kuro_domain::category::model::Category;

/// The rows of the dialog for a set of entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryDialog {
    entries: Vec<i64>,
    rows: Vec<(Category, Choice)>,
}

impl CategoryDialog {
    /// `memberships[i]` holds the visible category ids of `entries[i]`. The Default category is never offered.
    pub fn new(entries: Vec<i64>, categories: &[Category], memberships: &[Vec<i64>]) -> Self {
        let offered: Vec<&Category> = categories.iter().filter(|c| !c.is_system()).collect();
        let ids: Vec<i64> = offered.iter().map(|c| c.id).collect();
        let states = bulk::initial_states(memberships, &ids);
        let rows = offered
            .into_iter()
            .map(|c| {
                let initial = states.iter().find(|(id, _)| *id == c.id).map(|(_, s)| *s);
                let choice = initial.map_or(Choice::Unchecked, bulk::start_value);
                (c.clone(), choice)
            })
            .collect();
        Self { entries, rows }
    }

    #[cfg(test)]
    /// The entries the dialog changes.
    pub fn entries(&self) -> &[i64] {
        &self.entries
    }

    /// The rows in display order.
    pub fn rows(&self) -> &[(Category, Choice)] {
        &self.rows
    }

    #[cfg(test)]
    /// True when the library has no category besides Default: the dialog shows "edit categories" instead.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// A click on a row advances its tri-state (`CheckboxState.next()`).
    pub fn click(&mut self, category_id: i64) {
        if let Some((_, choice)) = self.rows.iter_mut().find(|(c, _)| c.id == category_id) {
            *choice = bulk::click(*choice);
        }
    }

    /// The `(include, exclude)` id lists the OK button sends. A mixed row left untouched is in neither.
    pub fn confirm(&self) -> (Vec<i64>, Vec<i64>) {
        let choices: Vec<(i64, Choice)> = self.rows.iter().map(|(c, choice)| (c.id, *choice)).collect();
        bulk::confirm(&choices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(id: i64) -> Category {
        Category { id, name: format!("c{id}"), order: id, flags: 0, hidden: false }
    }

    fn choice_of(dialog: &CategoryDialog, id: i64) -> Choice {
        dialog.rows().iter().find(|(c, _)| c.id == id).map(|(_, ch)| *ch).expect("row")
    }

    #[test]
    fn common_checked_mixed_untouched_and_others_unchecked_with_default_never_offered() {
        let cats = [cat(0), cat(1), cat(2), cat(3)];
        let memberships = vec![vec![1, 2], vec![1]];
        let dialog = CategoryDialog::new(vec![10, 11], &cats, &memberships);
        assert_eq!(dialog.rows().len(), 3, "Default is not offered");
        assert_eq!(choice_of(&dialog, 1), Choice::Checked);
        assert_eq!(choice_of(&dialog, 2), Choice::Untouched);
        assert_eq!(choice_of(&dialog, 3), Choice::Unchecked);
    }

    #[test]
    fn clicks_cycle_and_confirm_splits_the_lists() {
        let cats = [cat(1), cat(2), cat(3)];
        let memberships = vec![vec![1, 2], vec![1]];
        let mut dialog = CategoryDialog::new(vec![10, 11], &cats, &memberships);
        assert_eq!(dialog.confirm(), (vec![1], vec![3]));
        dialog.click(2);
        assert_eq!(choice_of(&dialog, 2), Choice::Exclude);
        dialog.click(2);
        assert_eq!(choice_of(&dialog, 2), Choice::Include);
        dialog.click(3);
        assert_eq!(dialog.confirm(), (vec![1, 2, 3], vec![]));
    }

    #[test]
    fn a_library_without_categories_gives_an_empty_dialog() {
        let dialog = CategoryDialog::new(vec![10], &[cat(0)], &[vec![0]]);
        assert!(dialog.is_empty());
        assert_eq!(dialog.entries(), &[10]);
    }
}
