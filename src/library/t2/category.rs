//! T2-CAT-01 to T2-CAT-04 and T2-CAT-08: the change-categories dialog and the membership delta.

use kuro_domain::category::bulk::{Choice, Initial, initial_states};
use kuro_domain::category::model::Category;
use kuro_domain::category::service::CategoryService;
use kuro_domain::error::DomainError;

use crate::library::actions::next_membership;
use crate::library::category_dialog::CategoryDialog;

fn cat(id: i64, name: &str) -> Category {
    Category { id, name: name.into(), order: id, flags: 0x40, hidden: false }
}

#[test]
fn t2_cat_01_the_default_category_is_never_offered() {
    let default = Category { id: 0, name: String::new(), order: -1, flags: 0x40, hidden: false };
    let dialog = CategoryDialog::new(vec![1], &[default, cat(1, "A")], &[vec![]]);
    assert_eq!(dialog.rows().iter().map(|(c, _)| c.id).collect::<Vec<_>>(), vec![1]);
}

#[test]
fn t2_cat_02_a_hidden_membership_survives_a_delta() {
    // the entry is in visible category 1 and hidden category 9; add 2, remove 1
    assert_eq!(next_membership(&[1, 9], &[2], &[1]), vec![2, 9]);
}

#[test]
fn t2_cat_03_a_mixed_row_left_alone_changes_neither_entry() {
    // entry A is in {1}, entry B in {2}: both categories are mixed, and nothing is clicked
    let dialog = CategoryDialog::new(vec![10, 11], &[cat(1, "A"), cat(2, "B"), cat(3, "C")], &[vec![1], vec![2]]);
    let (include, exclude) = dialog.confirm();
    assert!(!include.contains(&1) && !exclude.contains(&1), "mixed category 1 is in neither list");
    assert!(!include.contains(&2) && !exclude.contains(&2));
    assert_eq!(next_membership(&[1], &include, &exclude).contains(&1), true);
    assert_eq!(next_membership(&[2], &include, &exclude).contains(&2), true);
}

#[test]
fn t2_cat_04_common_and_mixed_categories() {
    let states = initial_states(&[vec![1, 2], vec![2]], &[1, 2]);
    assert_eq!(states, vec![(1, Initial::Mixed), (2, Initial::Checked)]);
    let dialog = CategoryDialog::new(vec![10, 11], &[cat(1, "A"), cat(2, "B")], &[vec![1, 2], vec![2]]);
    assert_eq!(dialog.rows()[0].1, Choice::Untouched);
    assert_eq!(dialog.rows()[1].1, Choice::Checked);
}

/// T2-CAT-08, a compile check: creating, renaming and deleting a category all fail with the one `DomainError`.
fn t2_cat_08_every_category_write_has_one_error_type(svc: &CategoryService) {
    fn same_error<A, B, C, E>(
        _: impl std::future::Future<Output = Result<A, E>>,
        _: impl std::future::Future<Output = Result<B, E>>,
        _: impl std::future::Future<Output = Result<C, E>>,
    ) {
    }
    same_error::<_, _, _, DomainError>(svc.create("x"), svc.rename(1, "y"), svc.delete(1));
}
