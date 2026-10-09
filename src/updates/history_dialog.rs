//! The confirm dialog of the History screen (T-42a-018, T-42a-024): a text, an optional checkbox whose state reaches
//! the confirm callback, Cancel and a confirm button. `show_dialog_then` of the shell passes no value to its callback,
//! so the remove dialog with its "Reset all" checkbox is built here.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, CheckButton, Label, Orientation as O};

/// What the dialog says. `checkbox` is the label of the optional switch.
pub struct ConfirmCopy {
    pub title: String,
    pub text: String,
    pub checkbox: Option<String>,
    pub confirm: String,
    pub cancel: String,
}

/// Shows the dialog. `on_ok` gets the checkbox state (false when there is no checkbox). `on_close` runs once the
/// dialog is gone, whichever button closed it, so the screen can drop its dialog state.
pub fn show(copy: ConfirmCopy, destructive: bool, on_ok: impl Fn(bool) + 'static, on_close: impl Fn() + 'static) {
    let col = GBox::new(O::Vertical, 12);
    crate::ui::margins(&col, 18);
    col.append(&Label::builder().label(&copy.text).wrap(true).xalign(0.0).max_width_chars(50).build());
    let check = copy.checkbox.as_deref().map(|label| CheckButton::with_label(label));
    if let Some(check) = &check {
        col.append(check);
    }
    let buttons = GBox::new(O::Horizontal, 6);
    buttons.set_halign(Align::End);
    buttons.set_margin_top(6);
    col.append(&buttons);
    let win = crate::ui::modal(&Label::builder().label(&copy.title).css_classes(["title"]).build().upcast(), &col, 420, -1);
    win.connect_close_request(move |_| {
        on_close();
        gtk::glib::Propagation::Proceed
    });

    let cancel = Button::with_label(&copy.cancel);
    let w = win.clone();
    cancel.connect_clicked(move |_| w.close());
    buttons.append(&cancel);

    let confirm = Button::with_label(&copy.confirm);
    confirm.add_css_class(if destructive { "destructive-action" } else { "suggested-action" });
    let on_ok = Rc::new(on_ok);
    let w = win.clone();
    confirm.connect_clicked(move |_| {
        on_ok(check.as_ref().is_some_and(CheckButton::is_active));
        w.close();
    });
    buttons.append(&confirm);
    win.present();
}
