//! The playback speed button and its menu. The menu offers six presets, and the button shows the speed that plays now.
//! The speed is written to mpv, which keeps it while the page plays on. Label text comes from `kuro_player::speed`.

use std::rc::Rc;

use kuro_player::speed::speed_label;

use super::super::Shared;
use super::{Choice, menu, section};

/// The speeds the menu offers, slowest first.
pub const PRESETS: [f32; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

/// The speed button. Its label follows the playing speed through [`refresh`].
pub fn button(shared: &Shared) -> gtk::MenuButton {
    let button = menu("Playback speed", {
        let shared = Rc::clone(shared);
        move |column, popover| fill(column, popover, &shared)
    });
    refresh(&button, shared);
    button
}

/// Shows the playing speed on `button`. Called on every tick, so the text changes only when it differs.
pub fn refresh(button: &gtk::MenuButton, shared: &Shared) {
    let text = speed_label(playing(shared) as f32);
    if button.label().as_deref() != Some(text.as_str()) {
        button.set_label(&text);
    }
}

fn fill(column: &gtk::Box, popover: &gtk::Popover, shared: &Shared) {
    section(column, None, choices(shared), popover);
}

/// The menu's rows: the presets with the playing one marked. The video's right-click menu lists the same.
pub fn choices(shared: &Shared) -> Vec<Choice> {
    let playing = playing(shared);
    PRESETS
        .iter()
        .map(|&speed| {
            let shared = Rc::clone(shared);
            Choice {
                title: speed_label(speed),
                subtitle: None,
                current: is_current(speed, playing),
                enabled: true,
                pick: Box::new(move || set_speed(&shared, speed)),
            }
        })
        .collect()
}

/// The speed mpv plays at; 1.0 before the file has one.
fn playing(shared: &Shared) -> f64 {
    shared.borrow().playback.mpv().get_f64("speed").unwrap_or(1.0)
}

fn set_speed(shared: &Shared, speed: f32) {
    if let Err(e) = shared.borrow().playback.mpv().set_property_f64("speed", f64::from(speed)) {
        tracing::warn!(error = %e, "player: the speed was not set");
    }
}

/// Whether `preset` is the playing speed. mpv stores a float, so the check allows a rounding error.
fn is_current(preset: f32, playing: f64) -> bool {
    (f64::from(preset) - playing).abs() < 0.005
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_are_the_six_speeds_in_order() {
        assert_eq!(PRESETS, [0.5, 0.75, 1.0, 1.25, 1.5, 2.0]);
    }

    #[test]
    fn the_check_follows_the_playing_speed_within_rounding() {
        assert!(is_current(1.25, 1.25));
        assert!(is_current(1.0, 1.0004));
        assert!(!is_current(1.25, 1.0));
    }
}
