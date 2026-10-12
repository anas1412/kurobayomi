//! The confirm dialog of the History screen (T-42a-018, T-42a-024): a text, an optional checkbox whose state reaches
//! the confirm callback, Cancel and a confirm button. `show_dialog_then` of the shell passes no value to its callback,
//! so the remove dialog with its "Reset all" checkbox is built here. Escape closes the dialog. A modal window has no scrim, so a click outside it cannot cancel; the parent window is blocked instead.

use std::cell::OnceCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Box as GBox, CheckButton, Orientation as O};

use crate::ui::{DialogButton, Role};

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
    col.append(&crate::ui::message(&copy.text));
    let check = copy.checkbox.as_deref().map(|label| CheckButton::with_label(label));
    if let Some(check) = &check {
        col.append(check);
    }
    // a second click while the window is closing does not confirm again (a double click deletes once)
    let confirmed = std::cell::Cell::new(false);
    let confirm = DialogButton::new(&copy.confirm, if destructive { Role::Destructive } else { Role::Suggested }, move || {
        if confirmed.replace(true) {
            return;
        }
        on_ok(check.as_ref().is_some_and(CheckButton::is_active));
    });
    let win = crate::ui::dialog(&copy.title, &col, 420, vec![DialogButton::cancel(&copy.cancel), confirm]);
    win.connect_close_request(move |_| {
        on_close();
        gtk::glib::Propagation::Proceed
    });
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
    let checks: Vec<(i64, CheckButton)> = rows
        .into_iter()
        .map(|(id, name, on)| {
            let check = CheckButton::with_label(&name);
            check.set_active(on);
            col.append(&check);
            (id, check)
        })
        .collect();
    let scroll = gtk::ScrolledWindow::builder()
        .child(&col)
        .propagate_natural_height(true)
        .max_content_height(360)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    // Edit categories closes the dialog first and then runs `on_edit`, so it reaches its window through this cell
    let this: Rc<OnceCell<gtk::glib::WeakRef<gtk::Window>>> = Rc::default();
    let closing = Rc::clone(&this);
    let edit_button = DialogButton::new(edit, Role::Normal, move || {
        if let Some(win) = closing.get().and_then(|w| w.upgrade()) {
            win.close();
        }
        on_edit();
    });
    let ok_button = DialogButton::new(ok, Role::Suggested, move || {
        on_ok(checks.iter().filter(|(_, check)| check.is_active()).map(|(id, _)| *id).collect());
    });
    let win = crate::ui::dialog(&title, &scroll, 420, vec![DialogButton::cancel(cancel), edit_button, ok_button]);
    let _ = this.set(win.downgrade());
    win.present();
}

/// A message with one button per label, in order, the last one the main button. `on_pick` gets the index of the
/// pressed button; a dismissal calls nothing. Used by the duplicate dialog of an add from History.
pub fn pick(title: String, text: String, labels: Vec<String>, on_pick: impl Fn(usize) + 'static) {
    let on_pick = Rc::new(on_pick);
    let last = labels.len().saturating_sub(1);
    let buttons = labels
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            let on_pick = Rc::clone(&on_pick);
            DialogButton::new(label, if index == last { Role::Suggested } else { Role::Normal }, move || on_pick(index))
        })
        .collect();
    crate::ui::dialog(&title, &crate::ui::message(&text), 420, buttons).present();
}
