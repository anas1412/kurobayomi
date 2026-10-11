//! The load problems of the extension packages (T-18a-054): each package folder's last load error, shown as a list under
//! the extensions so a user can see why a package did not load. The host keeps them in the registry by folder name.

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

/// One line of the list: the folder and the reason.
pub fn line(folder: &str, message: &str) -> String {
    format!("{folder}: {message}")
}

/// The heading and one row per problem, or `None` when every package loaded.
pub fn section(problems: &[(String, String)]) -> Option<gtk::Widget> {
    if problems.is_empty() {
        return None;
    }
    let column = gtk::Box::new(gtk::Orientation::Vertical, 4);
    // the list column (`ui::column`) carries the page margins
    column.set_margin_top(8);
    column.append(&gtk::Label::builder().label(tr(MsgKey::ext_load_problems)).xalign(0.0).css_classes(["heading"]).build());
    for (folder, message) in problems {
        column.append(&gtk::Label::builder().label(line(folder, message)).xalign(0.0).wrap(true).selectable(true).css_classes(["dim-label", "caption"]).build());
    }
    Some(column.upcast())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_problem_reads_as_its_folder_and_reason() {
        assert_eq!(line("broken", "component.wasm is missing"), "broken: component.wasm is missing");
        assert_eq!(tr(MsgKey::ext_load_problems), "Load problems");
    }

    #[test]
    fn no_problems_no_section() {
        assert!(section(&[]).is_none());
    }
}
