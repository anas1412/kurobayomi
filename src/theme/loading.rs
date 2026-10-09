//! The `LoadingScreen` equivalent (T-49b-027): a centred spinner that appears only when loading
//! lasts `SHOW_AFTER_MS` or more, so a fast load does not flicker.

use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;

/// The delay before the spinner appears. Proposal from the plan: it avoids flicker on fast loads.
pub const SHOW_AFTER_MS: u64 = 300;

/// True once a load has run long enough to show the spinner.
pub fn should_show(elapsed_ms: u64) -> bool {
    elapsed_ms >= SHOW_AFTER_MS
}

/// A centred spinner that fills its parent. It is hidden at first and made visible after
/// `SHOW_AFTER_MS`. If the page is dropped before then, the timer does nothing.
pub fn delayed_spinner() -> gtk::Spinner {
    let spinner = gtk::Spinner::builder()
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .hexpand(true)
        .vexpand(true)
        .spinning(true)
        .visible(false)
        .build();
    let weak = spinner.downgrade();
    glib::timeout_add_local_once(Duration::from_millis(SHOW_AFTER_MS), move || {
        if let Some(spinner) = weak.upgrade() {
            spinner.set_visible(true);
        }
    });
    spinner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_waits_for_the_delay() {
        assert!(!should_show(0));
        assert!(!should_show(299));
        assert!(should_show(300));
        assert!(should_show(5_000));
    }
}
