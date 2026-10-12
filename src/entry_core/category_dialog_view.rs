//! The change-categories dialog window: one check button per user category and an OK button. OK hands the checked ids
//! to `on_ok`; any other way out (Escape, the close button) calls `on_dismiss` and writes nothing. The rules are
//! `dialog::CategoryDialog`.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

use crate::entry_core::dialog::CategoryDialog;
use crate::ui;

/// Opens the dialog over `model`. `on_ok` runs with the checked ids before the window closes; `on_dismiss` runs on
/// every close that was not OK. The window is returned, so a page can keep it in its one dialog slot.
pub fn open(model: CategoryDialog, on_ok: impl Fn(Vec<i64>) + 'static, on_dismiss: impl Fn() + 'static) -> gtk::Window {
    open_with(model, move |m| on_ok(m.checked_ids()), on_dismiss)
}

/// [`open`] where `on_ok` gets the whole model (for a caller that confirms through `CategoryDialog::confirm`).
pub fn open_with(model: CategoryDialog, on_ok: impl Fn(&CategoryDialog) + 'static, on_dismiss: impl Fn() + 'static) -> gtk::Window {
    let model = Rc::new(RefCell::new(model));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let manage = gtk::Button::builder().label(tr(MsgKey::action_edit_categories)).css_classes(["flat"]).halign(gtk::Align::Start).build();
    body.append(&manage);
    if model.borrow().items.is_empty() {
        body.append(&gtk::Label::builder().label("No categories yet.").css_classes(["dim-label"]).xalign(0.0).build());
    } else {
        let list = ui::boxed_list();
        for (i, item) in model.borrow().items.iter().enumerate() {
            let check = gtk::CheckButton::builder().label(&item.name).active(item.checked).margin_start(8).margin_end(8).build();
            let model = Rc::clone(&model);
            check.connect_toggled(move |_| model.borrow_mut().toggle(i));
            list.append(&check);
        }
        let scroll = gtk::ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(360)
            .build();
        body.append(&scroll);
    }
    let buttons = vec![
        ui::DialogButton::cancel(tr(MsgKey::action_cancel)),
        ui::DialogButton::new(tr(MsgKey::action_ok), ui::Role::Suggested, {
            let model = Rc::clone(&model);
            move || on_ok(&model.borrow())
        }),
    ];
    let window = ui::dialog_or_dismiss(&tr(MsgKey::action_edit_categories), &body, 360, buttons, on_dismiss);
    // a dismiss that opens the categories screen
    manage.connect_clicked({
        let window = window.clone();
        move |_| {
            window.close();
            ui::open("categories");
        }
    });
    window.present();
    window
}
