//! Buttons (T-49a-050): filled and text kinds, long-press, and the disabled container rules.
//!
//! Filled: primary container, minimum 58 x 40 px, pill radius; disabled is a 12% container and a 38%
//! text. Text: transparent, primary text, 12 px side padding, disabled 38% text. Hover adds the 1 dp
//! elevation as a shadow (`style.css`), the tonal model of the source is not ported.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;

use crate::theme::tokens::{DISABLED_ALPHA, PAD_MEDIUM_SMALL};

pub const FILLED_MIN_WIDTH: i32 = 58;
pub const FILLED_MIN_HEIGHT: i32 = 40;
pub const TEXT_PAD_H: i32 = PAD_MEDIUM_SMALL as i32;
pub const DISABLED_CONTAINER_ALPHA: f32 = 0.12;
pub const HOVER_ELEVATION_DP: f32 = 1.0;

/// The two button kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Filled,
    Text,
}

impl Kind {
    /// The CSS class of the kind (rules in `style.css`).
    pub fn class(self) -> &'static str {
        match self {
            Kind::Filled => crate::theme::style::class::BUTTON_FILLED,
            Kind::Text => crate::theme::style::class::BUTTON_TEXT,
        }
    }
}

/// The text colour alpha for a control: full when enabled, `DISABLED_ALPHA` otherwise.
pub fn text_alpha(enabled: bool) -> f32 {
    if enabled { 1.0 } else { DISABLED_ALPHA }
}

/// Builds a button. A long press (`on_long_press`) fires on the press and suppresses the click that
/// follows the release, as Compose's combined click does.
pub fn button(
    label: &str,
    kind: Kind,
    enabled: bool,
    on_click: impl Fn() + 'static,
    on_long_press: Option<impl Fn() + 'static>,
) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class(kind.class());
    button.set_sensitive(enabled);
    let suppress = Rc::new(Cell::new(false));
    if let Some(on_long_press) = on_long_press {
        // A new press clears a stale flag left by a long press that released outside the button.
        let clear = gtk::GestureClick::new();
        let suppress_clear = suppress.clone();
        clear.connect_pressed(move |_, _, _, _| suppress_clear.set(false));
        button.add_controller(clear);
        let press = gtk::GestureLongPress::new();
        let suppress_on_press = suppress.clone();
        press.connect_pressed(move |_, _, _| {
            suppress_on_press.set(true);
            on_long_press();
        });
        button.add_controller(press);
    }
    button.connect_clicked(move |_| {
        if suppress.replace(false) {
            return;
        }
        on_click();
    });
    button
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filled_minimum_size_and_disabled_levels() {
        assert_eq!((FILLED_MIN_WIDTH, FILLED_MIN_HEIGHT), (58, 40));
        assert!((DISABLED_CONTAINER_ALPHA - 0.12).abs() < 1e-6);
        assert_eq!(text_alpha(false), 0.38);
        assert_eq!(text_alpha(true), 1.0);
    }

    #[test]
    fn text_padding_is_12_and_hover_elevation_is_1_dp() {
        assert_eq!(TEXT_PAD_H, 12);
        assert_eq!(HOVER_ELEVATION_DP, 1.0);
    }

    #[test]
    fn each_kind_has_its_own_class() {
        assert_ne!(Kind::Filled.class(), Kind::Text.class());
    }
}
