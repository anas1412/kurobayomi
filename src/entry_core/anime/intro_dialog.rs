//! The skip-intro length dialog (T-39a-075, `SkipIntroLengthDialog`): a number of seconds from 0 to 255, Cancel and OK.
//! OK hands the value back; 0 turns the skip off. The start value is `viewer::dialog_start`. Thin widget.

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O, SpinButton};
use kuro_i18n::{MsgKey, tr};

use crate::ui;

pub fn open(start: i32, on_ok: impl Fn(i32) + 'static) -> gtk::Window {
    let wheel = SpinButton::with_range(0.0, 255.0, 1.0);
    wheel.set_value(f64::from(start));
    let cancel = Button::with_label(&tr(MsgKey::action_cancel));
    let ok = Button::builder().label(tr(MsgKey::action_ok)).css_classes(["suggested-action"]).build();
    let buttons = GBox::new(O::Horizontal, 8);
    buttons.set_halign(Align::End);
    buttons.append(&cancel);
    buttons.append(&ok);
    let col = ui::column();
    col.append(&wheel);
    col.append(&buttons);
    let window = ui::modal(Label::new(Some(&tr(MsgKey::pref_intro_length))).upcast_ref(), &col, 320, 160);
    cancel.connect_clicked({
        let window = window.clone();
        move |_| window.close()
    });
    ok.connect_clicked({
        let window = window.clone();
        move |_| {
            on_ok(wheel.value_as_int());
            window.close();
        }
    });
    window.present();
    window
}
