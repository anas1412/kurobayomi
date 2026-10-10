//! The Add and Rename dialog of the categories screen (T-37a-042, T2-CATSCR-01). The rules are
//! `category_screen::check_name`: this file only shows them. On every change of the entry the OK button follows
//! `NameCheck::confirm_enabled` and the helper line under the entry shows "*required" or the duplicate error. The
//! names to compare with include the hidden categories, because the caller passes every other user category.

use gtk::prelude::*;
use gtk::{Align, Orientation as O};

use crate::library::category_screen::{NameCheck, check_name};

/// Applies a name check to the OK button and the helper label.
fn show_check(check: &NameCheck, ok: &gtk::Button, helper: &gtk::Label) {
    ok.set_sensitive(check.confirm_enabled);
    helper.set_text(&check.helper.text());
    if check.helper.is_error() {
        helper.add_css_class("error");
    } else {
        helper.remove_css_class("error");
    }
}

// Not called yet: `src/live.rs` (`name_dialog`) still draws its own copy, which keeps OK enabled and reports an empty
// or duplicate name only after a click. That function delegates here in one call; the file belongs to another lane.
#[allow(dead_code)]
/// Opens the dialog. `others` are the names of the other categories (hidden ones too; the one being renamed is
/// not among them). `buttons` are the labels of the spec dialog: the last one is OK, the others close the dialog.
/// `on_ok` gets the trimmed name.
pub fn show(title: &str, buttons: &[String], initial: &str, others: Vec<String>, on_ok: impl Fn(String) + 'static) {
    let labels = if buttons.is_empty() { vec![kuro_i18n::tr(kuro_i18n::MsgKey::action_ok)] } else { buttons.to_vec() };
    let column = gtk::Box::new(O::Vertical, 12);
    crate::ui::margins(&column, 18);
    let entry = gtk::Entry::builder().text(initial).activates_default(true).build();
    let helper = gtk::Label::builder().xalign(0.0).wrap(true).css_classes(["caption"]).build();
    column.append(&entry);
    column.append(&helper);
    let row = gtk::Box::new(O::Horizontal, 6);
    row.set_halign(Align::End);
    column.append(&row);
    let heading = gtk::Label::builder().label(title).css_classes(["title"]).build();
    let window = crate::ui::modal(heading.upcast_ref(), &crate::ui::scrolled(&column), 420, -1);

    let others = std::rc::Rc::new(others);
    let on_ok = std::rc::Rc::new(on_ok);
    let last = labels.len().saturating_sub(1);
    let mut ok_button = None;
    for (i, label) in labels.iter().enumerate() {
        let button = gtk::Button::with_label(label);
        if i == last {
            button.add_css_class("suggested-action");
            ok_button = Some(button.clone());
            let (window, entry, on_ok) = (window.clone(), entry.clone(), on_ok.clone());
            button.connect_clicked(move |_| {
                let name = entry.text().trim().to_owned();
                if !name.is_empty() {
                    on_ok(name);
                    window.close();
                }
            });
        } else {
            let window = window.clone();
            button.connect_clicked(move |_| window.close());
        }
        row.append(&button);
    }
    let Some(ok) = ok_button else { return };
    let sync = {
        let (ok, helper, others) = (ok.clone(), helper.clone(), others.clone());
        move |name: &str| {
            let taken: Vec<&str> = others.iter().map(String::as_str).collect();
            show_check(&check_name(name, &taken), &ok, &helper);
        }
    };
    sync(&entry.text());
    entry.connect_changed(move |entry| sync(&entry.text()));
    window.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t2_catscr_01_the_ok_button_and_helper_follow_the_name_check() {
        crate::testing::with_display(|| {
            let ok = gtk::Button::new();
            let helper = gtk::Label::new(None);
            show_check(&check_name("", &["Action"]), &ok, &helper);
            assert!(!ok.is_sensitive(), "an empty name is refused");
            assert_eq!(helper.text().as_str(), "*required");
            assert!(!helper.has_css_class("error"));
            show_check(&check_name("Action", &["Action"]), &ok, &helper);
            assert!(!ok.is_sensitive(), "a name another category uses is refused");
            assert!(helper.has_css_class("error"));
            assert_eq!(helper.text().as_str(), "A category with this name already exists!");
            show_check(&check_name("action", &["Action"]), &ok, &helper);
            assert!(ok.is_sensitive(), "the match is exact: action is not Action");
            assert!(!helper.has_css_class("error"));
        });
    }
}
