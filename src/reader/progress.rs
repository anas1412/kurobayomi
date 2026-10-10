//! Loading indicator (T-43b-029, `ReaderProgressIndicator.kt`). A spinner while the page loads. Once `set_progress` is
//! called the spinner gives way to a determinate ring. The percentage is clamped to 0..=100 (E-31, D-5).

use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

use gtk::prelude::*;

/// Edge length of the indicator in pixels.
pub const INDICATOR_SIZE: i32 = 48;

/// The percentage a value shows: below 0 is 0 and above 100 is 100 (E-31).
pub fn clamp_progress(value: i32) -> u8 {
    value.clamp(0, 100) as u8
}

pub struct LoadingIndicator {
    root: gtk::Box,
    spinner: gtk::Spinner,
    ring: gtk::DrawingArea,
    progress: Rc<Cell<Option<u8>>>,
}

impl LoadingIndicator {
    pub fn new() -> Self {
        let progress: Rc<Cell<Option<u8>>> = Rc::new(Cell::new(None));
        let spinner = gtk::Spinner::builder().width_request(INDICATOR_SIZE).height_request(INDICATOR_SIZE).build();
        let ring = gtk::DrawingArea::builder().content_width(INDICATOR_SIZE).content_height(INDICATOR_SIZE).visible(false).build();
        let drawn = Rc::clone(&progress);
        ring.set_draw_func(move |_, cr, w, h| {
            let Some(percent) = drawn.get() else { return };
            let (cx, cy) = (f64::from(w) / 2.0, f64::from(h) / 2.0);
            let radius = f64::from(w.min(h)) / 2.0 - 3.0;
            cr.set_line_width(4.0);
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.25);
            cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
            let _ = cr.stroke();
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
            cr.arc(cx, cy, radius, -PI / 2.0, -PI / 2.0 + 2.0 * PI * f64::from(percent) / 100.0);
            let _ = cr.stroke();
        });
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_halign(gtk::Align::Center);
        root.set_valign(gtk::Align::Center);
        root.append(&spinner);
        root.append(&ring);
        root.set_visible(false);
        Self { root, spinner, ring, progress }
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    /// Shows the indicator and keeps the spinner turning while it is visible.
    pub fn show(&self) {
        self.root.set_visible(true);
        self.spinner.set_spinning(true);
    }

    pub fn hide(&self) {
        self.root.set_visible(false);
        self.spinner.set_spinning(false);
    }

    pub fn is_shown(&self) -> bool {
        self.root.is_visible()
    }

    /// Back to the spinner (a queued or loading page after a download was shown).
    pub fn show_spinner(&self) {
        self.progress.set(None);
        self.ring.set_visible(false);
        self.spinner.set_visible(true);
        self.show();
    }

    /// Switches to the determinate ring at `value` (clamped to 0..=100).
    pub fn set_progress(&self, value: i32) {
        self.progress.set(Some(clamp_progress(value)));
        self.spinner.set_spinning(false);
        self.spinner.set_visible(false);
        self.ring.set_visible(true);
        self.ring.queue_draw();
    }

    /// The shown percentage, or `None` while only the spinner is shown.
    pub fn progress(&self) -> Option<u8> {
        self.progress.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e31_the_percentage_is_clamped_to_zero_to_one_hundred() {
        assert_eq!(clamp_progress(150), 100);
        assert_eq!(clamp_progress(-5), 0);
        assert_eq!(clamp_progress(42), 42);
    }

    #[test]
    #[ignore = "needs a display; run alone: cargo test -p kurobayomi progress:: -- --ignored"]
    fn show_and_hide_toggle_and_the_ring_appears_only_after_set_progress() {
        if gtk::init().is_err() {
            eprintln!("no display: skipping the loading indicator");
            return;
        }
        let indicator = LoadingIndicator::new();
        assert!(!indicator.is_shown());
        assert_eq!(indicator.progress(), None);
        indicator.show();
        assert!(indicator.is_shown());
        indicator.set_progress(150);
        assert_eq!(indicator.progress(), Some(100));
        indicator.set_progress(-1);
        assert_eq!(indicator.progress(), Some(0));
        indicator.hide();
        assert!(!indicator.is_shown());
    }
}
