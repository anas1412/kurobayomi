//! The duplicate-add window (T-39a-047): three choices (Show anime, Migrate, Add anyway) and Cancel over `duplicate.rs`.
//! A choice closes the window first and then acts (T-DU1); Cancel only closes it. Nothing is written until a choice is
//! pressed.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::Label;
use kuro_i18n::{MsgKey, tr};

use super::duplicate::{BODY, Choice, ROWS, Step, TITLE, steps};
use crate::ui::{self, DialogButton, Role};

/// Opens the window. `act` runs the choice, after the window is closed.
pub fn open(act: impl Fn(Choice) + 'static) -> gtk::Window {
    let act: Rc<dyn Fn(Choice)> = Rc::new(act);
    let slot: Rc<RefCell<Option<gtk::Window>>> = Rc::new(RefCell::new(None));
    let body = Label::builder().label(tr(BODY)).wrap(true).xalign(0.0).build();
    let mut buttons = vec![DialogButton::cancel(tr(MsgKey::action_cancel))];
    for (choice, label, _) in ROWS {
        let (slot, act) = (Rc::clone(&slot), Rc::clone(&act));
        let role = if choice == Choice::AddAnyway { Role::Suggested } else { Role::Normal };
        buttons.push(DialogButton::new(tr(label), role, move || {
            // a press runs the steps of `duplicate::steps`: dismiss first, then act
            for step in steps(Some(choice)) {
                match step {
                    Step::Dismiss => {
                        if let Some(w) = slot.borrow().clone() {
                            w.close();
                        }
                    }
                    Step::Act(choice) => act(choice),
                }
            }
        }));
    }
    let dialog = ui::dialog(&tr(TITLE), &body, 360, buttons);
    *slot.borrow_mut() = Some(dialog.clone());
    dialog.present();
    dialog
}
