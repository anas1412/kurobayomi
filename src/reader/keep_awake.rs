//! Keep the screen on while the reader is open (T-043a-051). The inhibit is taken with the application and released
//! when the reader closes or the preference goes off.

use std::cell::{Cell, RefCell};

use gtk::prelude::*;

/// The inhibit is wanted while the preference is on and the reader is open.
pub fn should_inhibit(pref_on: bool, reader_open: bool) -> bool {
    pref_on && reader_open
}

/// Holds the inhibit cookie. [`KeepAwake::sync`] takes or releases it to match the wanted state.
#[derive(Default)]
pub struct KeepAwake {
    cookie: Cell<Option<u32>>,
    app: RefCell<Option<gtk::Application>>,
}

impl KeepAwake {
    pub fn sync(&self, window: &gtk::Window, want: bool) {
        let app = window.application();
        match (want, self.cookie.get()) {
            (true, None) => {
                if let Some(app) = app {
                    let cookie = app.inhibit(Some(window), gtk::ApplicationInhibitFlags::IDLE, Some("Reading manga"));
                    self.cookie.set(Some(cookie));
                    *self.app.borrow_mut() = Some(app);
                }
            }
            (false, Some(cookie)) => {
                self.release(cookie);
            }
            _ => {}
        }
    }

    fn release(&self, cookie: u32) {
        if let Some(app) = self.app.borrow_mut().take() {
            app.uninhibit(cookie);
        }
        self.cookie.set(None);
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        if let Some(cookie) = self.cookie.get() {
            self.release(cookie);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_inhibit_follows_the_preference_and_the_reader_being_open() {
        assert!(should_inhibit(true, true));
        assert!(!should_inhibit(false, true));
        assert!(!should_inhibit(true, false));
    }
}
