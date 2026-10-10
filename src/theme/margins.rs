//! Screen side margins (T-49ba-034, `values/dimens.xml` and `values-sw720dp/dimens.xml`): 16 px in a
//! window narrower than 720 px, 24 px from 720 px. The margin is read from the window width in
//! pixels, because the Linux window has no density-independent width to compare.

use std::cell::Cell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

/// The side margin below the breakpoint.
pub const COMPACT_MARGIN: i32 = 16;
/// The side margin from the breakpoint up.
pub const EXPANDED_MARGIN: i32 = 24;
/// The window width at which the wider margin starts.
pub const BREAKPOINT_PX: i32 = 720;

/// The side margin for a window `width_px` wide.
pub const fn screen_margin(width_px: i32) -> i32 {
    if width_px >= BREAKPOINT_PX { EXPANDED_MARGIN } else { COMPACT_MARGIN }
}

/// Calls `on_change` with the new margin whenever it changes. Checked once per frame, so resizes
/// that leave `default-width` unchanged are caught too (the same check as `size_class::attach`).
pub fn attach<F: Fn(i32) + 'static>(window: &impl IsA<gtk::Window>, on_change: F) {
    let last = Rc::new(Cell::new(-1));
    window.as_ref().add_tick_callback(move |widget, _clock| {
        let margin = screen_margin(widget.width());
        if last.get() != margin {
            last.set(margin);
            on_change(margin);
        }
        glib::ControlFlow::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_719_px_window_has_16_px_margins() {
        assert_eq!(screen_margin(719), 16);
        assert_eq!(screen_margin(0), 16);
    }

    #[test]
    fn a_720_px_window_has_24_px_margins() {
        assert_eq!(screen_margin(720), 24);
        assert_eq!(screen_margin(1920), 24);
    }
}
