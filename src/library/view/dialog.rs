//! The change-categories dialog (T-37a-029). The rules are in `category_dialog`; this file draws the rows and
//! forwards clicks. A row shows checked, unchecked, or mixed (inconsistent) as its state dictates.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_domain::category::bulk::Choice;

use crate::library::category_dialog::CategoryDialog;
use crate::ui::{DialogButton, Role};

/// Opens the dialog. `on_confirm` gets the `(include, exclude)` lists; `on_edit` opens the category screen.
pub fn show(dialog: CategoryDialog, on_confirm: impl Fn(Vec<i64>, Vec<i64>) + 'static, on_edit: impl Fn() + 'static) {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let cancel = || DialogButton::cancel("Cancel");

    if dialog.is_empty() {
        body.append(&gtk::Label::new(Some("You don't have any categories yet.")));
        let edit = gtk::Button::with_label("Edit categories");
        body.append(&edit);
        let window = crate::ui::dialog("Set categories", &body, 360, vec![cancel()]);
        let w = window.clone();
        edit.connect_clicked(move |_| {
            w.close();
            on_edit();
        });
        window.present();
        return;
    }

    let ids: Vec<i64> = dialog.rows().iter().map(|(c, _)| c.id).collect();
    let checks: Vec<gtk::CheckButton> = dialog
        .rows()
        .iter()
        .map(|(category, _)| {
            let check = gtk::CheckButton::with_label(category.name.as_str());
            check.set_can_target(false);
            check
        })
        .collect();
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    for check in &checks {
        list.append(&gtk::ListBoxRow::builder().child(check).build());
    }
    let state = Rc::new(RefCell::new(dialog));
    paint(&state.borrow(), &checks);
    let (state_click, checks_click) = (state.clone(), checks.clone());
    list.connect_row_activated(move |_, row| {
        let index = row.index();
        let Some(id) = usize::try_from(index).ok().and_then(|i| ids.get(i).copied()) else {
            return;
        };
        state_click.borrow_mut().click(id);
        paint(&state_click.borrow(), &checks_click);
    });
    let scroll = gtk::ScrolledWindow::builder()
        .child(&list)
        .propagate_natural_height(true)
        .max_content_height(360)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    body.append(&scroll);
    let ok = DialogButton::new("OK", Role::Suggested, move || {
        let (include, exclude) = state.borrow().confirm();
        on_confirm(include, exclude);
    });
    crate::ui::dialog("Set categories", &body, 360, vec![cancel(), ok]).present();
}

/// Sets each check to the state of its row. Mixed rows show inconsistent.
fn paint(dialog: &CategoryDialog, checks: &[gtk::CheckButton]) {
    for ((_, choice), check) in dialog.rows().iter().zip(checks) {
        let (active, mixed) = match choice {
            Choice::Checked | Choice::Include => (true, false),
            Choice::Unchecked | Choice::Exclude => (false, false),
            Choice::Untouched => (false, true),
        };
        check.set_inconsistent(mixed);
        check.set_active(active);
    }
}
