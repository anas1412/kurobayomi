//! When the control bar hides: after three seconds without mouse movement or key press, and never while one of its menus
//! is open. The header never hides. Pure: the GTK timer passes the idle time and acts on the answer.

use std::time::Duration;

/// The idle time after which the bar hides.
pub const HIDE_AFTER: Duration = Duration::from_secs(3);

/// `true` when the bar should hide: it has been idle for [`HIDE_AFTER`] and no menu of it is open.
pub fn hidden(idle: Duration, menu_open: bool) -> bool {
    !menu_open && idle >= HIDE_AFTER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_hides_after_three_idle_seconds() {
        assert!(!hidden(Duration::from_millis(2_999), false));
        assert!(hidden(Duration::from_secs(3), false));
        assert!(hidden(Duration::from_secs(60), false));
    }

    #[test]
    fn an_open_menu_keeps_the_bar() {
        assert!(!hidden(Duration::from_secs(60), true));
    }
}
