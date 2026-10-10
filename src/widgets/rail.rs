//! Sidebar geometry, the NavigationRail equivalent (T-49b-017): the numbers and the label rule that
//! `src/main.rs::sidebar` applies, and the window-width follower that drives the label rule.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;

use crate::theme::tokens::MEDIUM_WIDTH_WINDOW_SIZE;

/// Minimum sidebar width in px.
pub const MIN_WIDTH: i32 = 80;
/// Vertical padding of the item column in px.
pub const PAD_V: i32 = 4;
/// Spacing between items in px.
pub const ITEM_SPACING: i32 = 4;
/// Gap after the header slot in px.
pub const HEADER_GAP: i32 = 8;

/// Labels show at a window width of 600 dp and above; below it only the icons remain.
pub fn labels_visible(width_dp: f32) -> bool {
    width_dp >= MEDIUM_WIDTH_WINDOW_SIZE
}

/// Calls `on_narrow` with `true` when the window of `widget` becomes narrower than 600 dp, and with
/// `false` when it widens again. It fires only when that state flips.
pub fn follow_width(widget: &impl IsA<gtk::Widget>, on_narrow: impl Fn(bool) + 'static) {
    let on_narrow: Rc<dyn Fn(bool)> = Rc::new(on_narrow);
    let hook = move |widget: &gtk::Widget| {
        let Some(surface) = widget.native().and_then(|native| native.surface()) else { return };
        let narrow = Rc::new(Cell::new(false));
        let on_narrow = on_narrow.clone();
        surface.connect_layout(move |_, width, _| {
            let now = !labels_visible(width as f32);
            if narrow.replace(now) != now {
                on_narrow(now);
            }
        });
    };
    if widget.is_realized() {
        hook(widget.upcast_ref());
    } else {
        let done = Cell::new(false);
        widget.connect_realize(move |w| {
            if !done.replace(true) {
                hook(w.upcast_ref());
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_constants() {
        assert_eq!((MIN_WIDTH, PAD_V, ITEM_SPACING, HEADER_GAP), (80, 4, 4, 8));
    }

    #[test]
    fn labels_hide_below_600_dp() {
        assert!(!labels_visible(599.0));
        assert!(labels_visible(600.0));
        assert!(labels_visible(900.0));
    }
}
