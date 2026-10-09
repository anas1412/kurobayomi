//! The "Default skip intro length" dialog (task T-48a-022): a spin button from 0 to 255 seconds with an `s` suffix.
//! It starts at the stored value; OK writes a changed value, Cancel and Escape close without writing.
use gtk::glib;
use gtk::prelude::*;
use kuro_prefs::pref::Pref;

use super::seeking::INTRO_LENGTH_RANGE;

/// The spin button text for `seconds`.
pub fn spin_text(seconds: i32) -> String {
    format!("{seconds}s")
}

/// The seconds a typed spin text means (`"90"` and `"90s"` both), or `None` when it is not a number.
pub fn parse_spin(text: &str) -> Option<i32> {
    text.trim().trim_end_matches('s').trim().parse().ok()
}

/// What OK stores: the spin value clamped to the range, or `None` when it equals `stored` (nothing to write).
pub fn confirmed(stored: i32, spin: f64) -> Option<i32> {
    let value = (spin.round() as i32).clamp(*INTRO_LENGTH_RANGE.start(), *INTRO_LENGTH_RANGE.end());
    (value != stored).then_some(value)
}

/// Opens the dialog over the main window.
pub fn show(pref: Pref<i32>) {
    let stored = pref.get();
    let (low, high) = (f64::from(*INTRO_LENGTH_RANGE.start()), f64::from(*INTRO_LENGTH_RANGE.end()));
    let spin = gtk::SpinButton::with_range(low, high, 1.0);
    spin.set_value(f64::from(stored));
    spin.set_numeric(false);
    spin.connect_output(|s| {
        s.set_text(&spin_text(s.value_as_int()));
        glib::Propagation::Stop
    });
    spin.connect_input(|s| parse_spin(&s.text()).map(|v| Ok(f64::from(v))));
    let col = gtk::Box::new(gtk::Orientation::Vertical, 12);
    crate::ui::margins(&col, 18);
    col.append(&spin);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    buttons.set_halign(gtk::Align::End);
    col.append(&buttons);
    let title = crate::ui::screen("settings_player_gestures")
        .dialogs
        .into_iter()
        .find(|d| d.id == "intro_length")
        .map_or("Skip intro length".to_owned(), |d| d.title);
    let win = crate::ui::modal(&gtk::Label::builder().label(&title).css_classes(["title"]).build().upcast(), &col, 320, -1);
    let cancel = gtk::Button::with_label("Cancel");
    let ok = gtk::Button::builder().label("OK").css_classes(["suggested-action"]).build();
    let w = win.clone();
    cancel.connect_clicked(move |_| w.close());
    let w = win.clone();
    ok.connect_clicked(move |_| {
        spin.update();
        if let Some(value) = confirmed(stored, spin.value()) {
            if let Err(e) = pref.set(value) {
                tracing::warn!(error = %e, "the intro length was not saved");
            }
        }
        w.close();
    });
    buttons.append(&cancel);
    buttons.append(&ok);
    let key = gtk::EventControllerKey::new();
    let w = win.clone();
    key.connect_key_pressed(move |_, k, _, _| {
        if k == gtk::gdk::Key::Escape {
            w.close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    win.add_controller(key);
    win.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spin_text_has_the_s_suffix_and_parses_back() {
        assert_eq!(spin_text(85), "85s");
        assert_eq!(parse_spin("85s"), Some(85));
        assert_eq!(parse_spin(" 90 "), Some(90));
        assert_eq!(parse_spin("abc"), None);
    }

    #[test]
    fn ok_without_a_change_writes_nothing_and_values_are_clamped_to_0_to_255() {
        assert_eq!(confirmed(85, 85.0), None);
        assert_eq!(confirmed(85, 86.0), Some(86));
        assert_eq!(confirmed(85, 0.0), Some(0));
        assert_eq!(confirmed(85, 300.0), Some(255));
        assert_eq!(confirmed(255, 999.0), None, "clamped to the stored value, so unchanged");
        assert_eq!(confirmed(85, -4.0), Some(0));
    }
}
