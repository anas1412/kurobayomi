//! The fetch interval dialog (T-38a-028): "Smart update" with the expected update text, the wheel (debug builds only),
//! a Reset button, Cancel and OK. The rules are in `fetch_interval`; this file draws them.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O, SpinButton};
use kuro_i18n::{Arg, MsgKey, tr, tr_plural, trf};
use kuro_domain::entries::fetch_interval::MAX_INTERVAL;

use crate::entry_core::fetch_interval::{Body, RESET_WHEEL, body, initial_wheel, wheel_visible};
use crate::ui;

/// Opens the dialog for an entry with `interval` (its stored fetch interval) and `expected_next_update_ms`. OK calls
/// `on_ok` with the wheel value and Reset with 0; the page stores `-wheel` (0 is automatic).
pub fn open(interval: i32, expected_next_update_ms: Option<i64>, on_ok: impl Fn(i32) + 'static) {
    let now = chrono::Utc::now().timestamp_millis();
    let text = match body(interval, expected_next_update_ms, now) {
        Body::Expected(days) => trf(
            MsgKey::manga_interval_expected_update,
            &[("arg1", Arg::Str(&tr_plural(MsgKey::day, days, &[])))],
        ),
        Body::Unknown => tr(MsgKey::manga_interval_expected_update_null),
    };

    let wheel = SpinButton::with_range(0.0, f64::from(MAX_INTERVAL), 1.0);
    wheel.set_value(f64::from(initial_wheel(interval)));
    wheel.set_visible(wheel_visible(cfg!(debug_assertions), true));

    let reset = Button::with_label(&tr(MsgKey::action_reset));
    let cancel = Button::with_label(&tr(MsgKey::action_cancel));
    let ok = Button::builder().label(tr(MsgKey::action_ok)).css_classes(["suggested-action"]).build();
    let buttons = GBox::new(O::Horizontal, 8);
    buttons.set_halign(Align::End);
    for b in [&reset, &cancel, &ok] {
        buttons.append(b);
    }

    let col = ui::column();
    col.append(&Label::builder().label(text).wrap(true).xalign(0.0).build());
    col.append(&wheel);
    col.append(&buttons);
    let window = ui::modal(Label::new(Some(&tr(MsgKey::pref_library_update_smart_update))).upcast_ref(), &col, 400, 240);

    cancel.connect_clicked({
        let window = window.clone();
        move |_| window.close()
    });
    let on_ok = Rc::new(on_ok);
    reset.connect_clicked({
        let (window, on_ok) = (window.clone(), Rc::clone(&on_ok));
        move |_| {
            on_ok(RESET_WHEEL);
            window.close();
        }
    });
    ok.connect_clicked(move |_| {
        on_ok(wheel.value_as_int());
        window.close();
    });
}
