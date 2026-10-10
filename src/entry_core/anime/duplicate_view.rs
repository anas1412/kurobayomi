//! The duplicate-add window (T-39a-047): three rows (Show anime, Migrate, Add anyway) and Cancel over `duplicate.rs`. A
//! row closes the window first and then acts (T-DU1); Cancel only closes it. Nothing is written until a row is pressed.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O};
use kuro_i18n::{MsgKey, tr};

use super::duplicate::{BODY, Choice, ROWS, Step, TITLE, steps};
use crate::ui;

/// Opens the window. `act` runs the choice, after the window is closed.
pub fn open(act: impl Fn(Choice) + 'static) -> gtk::Window {
    let act: Rc<dyn Fn(Choice)> = Rc::new(act);
    let col = ui::column();
    col.append(&Label::builder().label(tr(BODY)).wrap(true).xalign(0.0).build());
    let dialog = ui::modal(Label::new(Some(&tr(TITLE))).upcast_ref(), &col, 360, -1);
    // a press runs the steps of `duplicate::steps`: dismiss first, then act
    let press = |choice: Option<Choice>| {
        let (dialog, act) = (dialog.clone(), Rc::clone(&act));
        move || {
            for step in steps(choice) {
                match step {
                    Step::Dismiss => dialog.close(),
                    Step::Act(choice) => act(choice),
                }
            }
        }
    };
    for (choice, label, icon) in ROWS {
        let content = GBox::new(O::Horizontal, 12);
        content.append(&gtk::Image::from_icon_name(icon));
        content.append(&Label::builder().label(tr(label)).xalign(0.0).hexpand(true).build());
        let button = Button::builder().child(&content).css_classes(["flat"]).build();
        let run = press(Some(choice));
        button.connect_clicked(move |_| run());
        col.append(&button);
    }
    let cancel = Button::with_label(&tr(MsgKey::action_cancel));
    cancel.set_halign(Align::End);
    let run = press(None);
    cancel.connect_clicked(move |_| run());
    col.append(&cancel);
    dialog.present();
    dialog
}
