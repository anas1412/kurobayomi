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
    let model = Rc::new(RefCell::new(model));
    let list = ui::boxed_list();
    for (i, item) in model.borrow().items.iter().enumerate() {
        let check = gtk::CheckButton::builder().label(&item.name).active(item.checked).margin_start(8).margin_end(8).build();
        let model = Rc::clone(&model);
        check.connect_toggled(move |_| model.borrow_mut().toggle(i));
        list.append(&check);
    }
    let ok = gtk::Button::builder().label(tr(MsgKey::action_ok)).css_classes(["suggested-action"]).build();
    let col = ui::column();
    col.append(&list);
    col.append(&ok);
    let title = gtk::Label::new(Some(&tr(MsgKey::action_edit_categories)));
    let window = ui::modal(title.upcast_ref(), &col, 360, 420);
    let confirmed = Rc::new(std::cell::Cell::new(false));
    ok.connect_clicked({
        let (window, model, confirmed) = (window.clone(), Rc::clone(&model), Rc::clone(&confirmed));
        move |_| {
            confirmed.set(true);
            on_ok(model.borrow().checked_ids());
            window.close();
        }
    });
    window.connect_close_request(move |_| {
        if !confirmed.get() {
            on_dismiss();
        }
        gtk::glib::Propagation::Proceed
    });
    window.present();
    window
}
