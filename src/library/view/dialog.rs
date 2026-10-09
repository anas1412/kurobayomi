//! The change-categories dialog (T-37a-029). The rules are in `category_dialog`; this file draws the rows and
//! forwards clicks. A row shows checked, unchecked, or mixed (inconsistent) as its state dictates.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_domain::category::bulk::Choice;

use crate::library::category_dialog::CategoryDialog;

/// Opens the dialog. `on_confirm` gets the `(include, exclude)` lists; `on_edit` opens the category screen.
pub fn show(dialog: CategoryDialog, on_confirm: impl Fn(Vec<i64>, Vec<i64>) + 'static, on_edit: impl Fn() + 'static) {
    let title = gtk::Label::new(Some("Set categories"));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.set_margin_start(16);
    body.set_margin_end(16);
    body.set_margin_top(12);
    body.set_margin_bottom(12);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let ok = gtk::Button::with_label("OK");
    ok.add_css_class("suggested-action");

    if dialog.is_empty() {
        body.append(&gtk::Label::new(Some("You don't have any categories yet.")));
        let edit = gtk::Button::with_label("Edit categories");
        body.append(&edit);
        buttons.append(&cancel);
        body.append(&buttons);
        let window = crate::ui::modal(&title.clone().upcast(), &body, 360, 200);
        let w = window.clone();
        edit.connect_clicked(move |_| {
            w.close();
            on_edit();
        });
        let w = window.clone();
        cancel.connect_clicked(move |_| w.close());
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
    body.append(&list);

    buttons.append(&cancel);
    buttons.append(&ok);
    body.append(&buttons);
    let window = crate::ui::modal(&title.clone().upcast(), &body, 360, 420);
    let w = window.clone();
    cancel.connect_clicked(move |_| w.close());
    let w = window.clone();
    ok.connect_clicked(move |_| {
        let (include, exclude) = state.borrow().confirm();
        w.close();
        on_confirm(include, exclude);
    });
    window.present();
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
