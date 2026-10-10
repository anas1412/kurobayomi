//! Clickable surface and `clickableNoIndication` (T-49b-021). The hit target is at least 48 px square
//! (CSS `min-width` and `min-height`); the visual size does not change.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;

use crate::theme::elevation::tint_at;
use crate::theme::style::class;
use crate::theme::tokens::Rgba;

/// Minimum hit target in px, on both axes.
pub const HIT_MIN: i32 = 48;

/// Decides whether a click runs. A long press suppresses the click that follows it, once.
#[derive(Default)]
pub struct ClickGate {
    long_pressed: Cell<bool>,
}

impl ClickGate {
    /// Records a long press. The release that follows must not count as a click.
    pub fn long_press(&self) {
        self.long_pressed.set(true);
    }

    /// True when a click should run. The first click after a long press returns false.
    pub fn click(&self) -> bool {
        !self.long_pressed.replace(false)
    }
}

/// The background of a surface: `tint` mixed in at `elevation` dp when the surface is tonal, the
/// surface colour itself when it is not (`None`).
pub fn background(surface: Rgba, tint: Rgba, elevation: Option<f32>) -> Rgba {
    match elevation {
        Some(dp) => tint_at(surface, tint, dp, true),
        None => surface,
    }
}

/// A clickable surface around `child`. With `indication == false` it has no hover or press feedback
/// (`clickableNoIndication`). `on_long_press` is optional; a long press suppresses the click after it.
pub fn clickable(
    child: &impl IsA<gtk::Widget>,
    indication: bool,
    on_click: impl Fn() + 'static,
    on_long_press: Option<Rc<dyn Fn()>>,
) -> gtk::Box {
    let surface = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    surface.append(child);
    surface.add_css_class(class::HIT_TARGET);
    surface.add_css_class(if indication { class::SURFACE } else { class::NOINDICATION });
    let gate = Rc::new(ClickGate::default());
    if let Some(on_long) = on_long_press {
        let long = gtk::GestureLongPress::new();
        let gate_long = gate.clone();
        long.connect_pressed(move |_, _, _| {
            gate_long.long_press();
            on_long();
        });
        surface.add_controller(long);
    }
    let click = gtk::GestureClick::new();
    click.set_button(1);
    click.connect_released(move |_, _, _, _| {
        if gate.click() {
            on_click();
        }
    });
    surface.add_controller(click);
    surface
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_press_suppresses_exactly_one_click() {
        let gate = ClickGate::default();
        assert!(gate.click());
        gate.long_press();
        assert!(!gate.click());
        assert!(gate.click());
    }

    #[test]
    fn a_surface_without_a_tint_is_never_tinted() {
        let surface = Rgba::argb(0xFFFE_FBFF);
        let tint = Rgba::argb(0xFF00_58CA);
        assert_eq!(background(surface, tint, None), surface);
        assert_eq!(background(surface, tint, Some(3.0)), Rgba::argb(0xFFE9_EEFB));
    }

    #[test]
    fn hit_target_is_48_px() {
        assert_eq!(HIT_MIN, 48);
    }
}
