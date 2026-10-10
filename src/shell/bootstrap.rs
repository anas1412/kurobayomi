//! Launch bootstrap: the first window waits for the migrations, and launch requests that arrive before the window
//! exists are queued in arrival order (T-36a-001, T-36a-015, T-53a-006).
//!
//! The splash timing (`keep_hidden`) is the Kotlin `keepOnScreen` rule from behaviour section 1.6.

use std::collections::VecDeque;
use std::time::Duration;

/// The window stays hidden at least this long (behaviour 1.6).
pub const MIN_SHOW: Duration = Duration::from_millis(500);
/// After this long the window is shown whether or not the first route is applied.
pub const MAX_WAIT: Duration = Duration::from_millis(5000);

/// Whether the splash keeps the window hidden at `elapsed`, given whether the first route was applied.
pub fn keep_hidden(elapsed: Duration, first_route_applied: bool) -> bool {
    elapsed <= MIN_SHOW || (!first_route_applied && elapsed <= MAX_WAIT)
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
    fn splash_timing_matches_the_kotlin_rule_at_the_listed_instants() {
        let ms = Duration::from_millis;
        assert!(keep_hidden(ms(0), false));
        assert!(keep_hidden(ms(499), false));
        assert!(keep_hidden(ms(500), true), "still hidden at the minimum when ready");
        assert!(!keep_hidden(ms(501), true));
        assert!(keep_hidden(ms(4999), false));
        assert!(!keep_hidden(ms(5001), false), "forced show at the cap");
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
