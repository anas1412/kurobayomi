//! Launch bootstrap: the first window waits for the migrations, the library page shows a loading state until they finish,
//! and launch requests that arrive before the window exists are queued in arrival order (T-36a-001, T-36a-015, T-53a-006,
//! T-53a-069).
//!
//! The Kotlin splash timers (500 ms minimum, 5000 ms maximum, 400 ms exit animation) are not ported: GTK shows the window
//! when it is ready, and the library page shows `LibraryState::Loading` until the migrator finishes. The crash screen is
//! owned by area 01.

use std::collections::VecDeque;

/// What the library page shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryState {
    /// The migrator is still running: a spinner, no list.
    Loading,
    /// The migrator finished: the library list.
    Ready,
}

/// Startup state on the GTK thread. The window is presented only after the migrations resolve.
#[derive(Debug)]
pub struct Bootstrap<T> {
    migrated: bool,
    built: bool,
    queue: VecDeque<T>,
}

impl<T> Default for Bootstrap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Bootstrap<T> {
    pub fn new() -> Self {
        Self { migrated: false, built: false, queue: VecDeque::new() }
    }

    /// The migration future has resolved (successfully or not, so the window can show an error).
    pub fn migration_done(&mut self) {
        self.migrated = true;
    }

    /// The first window may be presented.
    pub fn may_present(&self) -> bool {
        self.migrated
    }

    /// The library page shows the loading state until the migrator finishes.
    pub fn library_state(&self) -> LibraryState {
        if self.migrated { LibraryState::Ready } else { LibraryState::Loading }
    }

    /// A launch request arrived. Returns it back when the window exists, so the caller applies it now.
    /// Otherwise it is queued and returned later by `window_built`.
    pub fn request(&mut self, req: T) -> Option<T> {
        if self.built {
            return Some(req);
        }
        self.queue.push_back(req);
        None
    }

    /// The window is built: returns the queued requests in arrival order.
    pub fn window_built(&mut self) -> Vec<T> {
        self.built = true;
        self.queue.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_splash_timers_are_ported() {
        let source = include_str!("bootstrap.rs");
        let code = &source[..source.find("#[cfg(test)]").unwrap()];
        for timer in ["MIN_SHOW", "MAX_WAIT", "keep_hidden", "Duration"] {
            assert!(!code.lines().filter(|l| !l.trim_start().starts_with("//")).any(|l| l.contains(timer)), "{timer}");
        }
    }

    #[test]
    fn the_library_page_loads_until_the_migrator_finishes() {
        let mut boot: Bootstrap<&str> = Bootstrap::new();
        assert_eq!(boot.library_state(), LibraryState::Loading);
        boot.migration_done();
        assert_eq!(boot.library_state(), LibraryState::Ready);
    }

    #[test]
    fn window_stays_hidden_until_migration_resolves() {
        let mut boot: Bootstrap<&str> = Bootstrap::new();
        assert!(!boot.may_present());
        boot.migration_done();
        assert!(boot.may_present());
    }

    #[test]
    fn requests_before_the_window_are_queued_then_drained_in_order() {
        let mut boot = Bootstrap::new();
        assert_eq!(boot.request("first"), None);
        assert_eq!(boot.request("second"), None);
        assert_eq!(boot.window_built(), vec!["first", "second"]);
        assert!(boot.window_built().is_empty(), "drained once");
    }

    #[test]
    fn requests_after_the_window_come_back_for_immediate_apply() {
        let mut boot = Bootstrap::new();
        boot.window_built();
        assert_eq!(boot.request("now"), Some("now"));
    }
}
