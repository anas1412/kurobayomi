//! Focus on open (T-49b-031, `Modifier.kt` showSoftKeyboard). A dialog or page takes the focus the
//! first time it is shown. Showing it again, or re-rendering it, does not move the focus.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;

/// Remembers whether the first-show focus has been given. Pure, so the once-only rule is tested
/// without a display.
#[derive(Debug, Default)]
pub struct FocusOnce(Cell<bool>);

impl FocusOnce {
    /// True the first time it is asked, false after that.
    pub fn take(&self) -> bool {
        !self.0.replace(true)
    }
}

/// Grabs the focus on the first map of `widget` only. The closure holds the widget weakly.
pub fn focus_on_open(widget: &impl IsA<gtk::Widget>) {
    let once = Rc::new(FocusOnce::default());
    let target = widget.as_ref().downgrade();
    widget.as_ref().connect_map(move |_| {
        if once.take() {
            if let Some(w) = target.upgrade() {
                w.grab_focus();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // T-49b-031 / M3: the first show focuses, later shows do not.
    #[test]
    fn only_the_first_show_takes_the_focus() {
        let once = FocusOnce::default();
        assert!(once.take());
        assert!(!once.take());
        assert!(!once.take());
    }
}
