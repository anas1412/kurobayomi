//! The confirm dialog of the History screen (T-42a-018, T-42a-024): a text, an optional checkbox whose state reaches
//! the confirm callback, Cancel and a confirm button. `show_dialog_then` of the shell passes no value to its callback,
//! so the remove dialog with its "Reset all" checkbox is built here. Cancel is the default button and Escape closes
//! the dialog. A modal window has no scrim, so a click outside it cannot cancel; the parent window is blocked instead.

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
    // Cancel is the default button, so Enter never confirms a destructive action; Escape closes the dialog
    win.set_default_widget(Some(&cancel));
    GtkWindowExt::set_focus(&win, Some(&cancel));
    let keys = gtk::EventControllerKey::new();
    let w = win.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            w.close();
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    win.add_controller(keys);

    let confirm = Button::with_label(&copy.confirm);
    confirm.add_css_class(if destructive { "destructive-action" } else { "suggested-action" });
    let on_ok = Rc::new(on_ok);
    let w = win.clone();
    // a second click while the window is closing does not confirm again (a double click deletes once)
    let confirmed = std::cell::Cell::new(false);
    confirm.connect_clicked(move |_| {
        if confirmed.replace(true) {
            return;
        }
        on_ok(check.as_ref().is_some_and(CheckButton::is_active));
        w.close();
    });
    buttons.append(&confirm);
    win.present();
}

/// The category chooser of an add from History (T-42a-019): one check per user category, the entry's current links
/// checked, then OK, Cancel and Edit categories. `on_ok` gets the checked ids; `on_edit` runs after the dialog closes.
pub fn choose_categories(
    title: String,
    rows: Vec<(i64, String, bool)>,
    labels: [String; 3],
    on_ok: impl Fn(Vec<i64>) + 'static,
    on_edit: impl Fn() + 'static,
) {
    let [ok, cancel, edit] = labels;
    let col = GBox::new(O::Vertical, 4);
    crate::ui::margins(&col, 18);
    let checks: Vec<(i64, CheckButton)> = rows
        .into_iter()
        .map(|(id, name, on)| {
            let check = CheckButton::with_label(&name);
            check.set_active(on);
            col.append(&check);
            (id, check)
        })
        .collect();
    let buttons = GBox::new(O::Horizontal, 6);
    buttons.set_halign(Align::End);
    buttons.set_margin_top(6);
    col.append(&buttons);
    let win = crate::ui::modal(&Label::builder().label(&title).css_classes(["title"]).build().upcast(), &col, 420, -1);

    let cancel_button = Button::with_label(&cancel);
    let w = win.clone();
    cancel_button.connect_clicked(move |_| w.close());
    buttons.append(&cancel_button);

    let edit_button = Button::with_label(&edit);
    let (w, on_edit) = (win.clone(), Rc::new(on_edit));
    edit_button.connect_clicked(move |_| {
        w.close();
        on_edit();
    });
    buttons.append(&edit_button);

    let ok_button = Button::with_label(&ok);
    ok_button.add_css_class("suggested-action");
    let (w, on_ok) = (win.clone(), Rc::new(on_ok));
    ok_button.connect_clicked(move |_| {
        let picked = checks.iter().filter(|(_, check)| check.is_active()).map(|(id, _)| *id).collect();
        w.close();
        on_ok(picked);
    });
    buttons.append(&ok_button);
    win.set_default_widget(Some(&ok_button));
    win.present();
}

/// A message with one button per label, in order. `on_pick` gets the index of the pressed button; a dismissal calls
/// nothing. Used by the duplicate dialog of an add from History.
pub fn pick(title: String, text: String, labels: Vec<String>, on_pick: impl Fn(usize) + 'static) {
    let col = GBox::new(O::Vertical, 12);
    crate::ui::margins(&col, 18);
    col.append(&Label::builder().label(&text).wrap(true).xalign(0.0).max_width_chars(50).build());
    let buttons = GBox::new(O::Vertical, 6);
    col.append(&buttons);
    let win = crate::ui::modal(&Label::builder().label(&title).css_classes(["title"]).build().upcast(), &col, 420, -1);
    let on_pick = Rc::new(on_pick);
    let last = labels.len().saturating_sub(1);
    for (index, label) in labels.iter().enumerate() {
        let button = Button::with_label(label);
        let (w, on_pick) = (win.clone(), Rc::clone(&on_pick));
        button.connect_clicked(move |_| {
            w.close();
            on_pick(index);
        });
        buttons.append(&button);
        if index == last {
            win.set_default_widget(Some(&button));
        }
    }
    win.present();
}
