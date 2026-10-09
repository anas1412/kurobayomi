//! T2-CATSCR-01 to T2-CATSCR-03: the category management screen's rules.

use kuro_domain::category::model::Category;

use crate::library::category_screen::{Dialog, Helper, after_refresh, check_name, management_list};

fn cat(id: i64, name: &str, hidden: bool) -> Category {
    Category { id, name: name.into(), order: id, flags: 0, hidden }
}

#[test]
fn t2_catscr_01_add_is_off_for_an_empty_or_existing_name_and_the_helper_switches() {
    let taken = ["Action", "Drama"];
    let empty = check_name("", &taken);
    assert!(!empty.confirm_enabled);
    assert_eq!(empty.helper, Helper::Required, "an empty name is not an error");
    let dup = check_name("Action", &taken);
    assert!(!dup.confirm_enabled);
    assert_eq!(dup.helper, Helper::Exists);
    assert!(dup.helper.is_error());
    let fresh = check_name("Comedy", &taken);
    assert!(fresh.confirm_enabled);
    assert_eq!(fresh.helper, Helper::Required);
    assert!(check_name("action", &taken).confirm_enabled, "the match is exact (E-40)");
    assert_eq!(Helper::Required.text(), "*required");
    assert_eq!(Helper::Exists.text(), "A category with this name already exists!");
}

#[test]
fn t2_catscr_02_an_open_dialog_survives_a_refresh_while_its_category_exists() {
    let list = [cat(1, "A", false), cat(2, "B", false)];
    let rename = Some(Dialog::Rename { id: 2, typed: "Bee".into() });
    assert_eq!(after_refresh(rename.clone(), &list), rename, "the typed text is kept");
    assert_eq!(after_refresh(Some(Dialog::Add), &list), Some(Dialog::Add));
    let after_delete = [cat(1, "A", false)];
    assert_eq!(after_refresh(rename, &after_delete), None, "its category is gone");
    assert_eq!(after_refresh(Some(Dialog::Delete { id: 1 }), &after_delete), Some(Dialog::Delete { id: 1 }));
    assert_eq!(after_refresh(None, &list), None);
}

#[test]
fn t2_catscr_03_the_hidden_setting_decides_what_the_management_list_shows() {
    let list = [cat(0, "", false), cat(1, "A", false), cat(2, "B", true), cat(3, "C", false)];
    let ids = |hide| management_list(&list, hide).iter().map(|c| c.id).collect::<Vec<_>>();
    assert_eq!(ids(false), vec![1, 2, 3], "hidden ones show, the system category never does");
    assert_eq!(ids(true), vec![1, 3]);
}
