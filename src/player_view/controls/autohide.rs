//! When the control bar hides: after three seconds without mouse movement or key press, or after 2.5 seconds in
//! fullscreen (where the pointer hides with it), and never while one of its menus is open. The header hides only in
//! fullscreen (`player_view::watch_fullscreen`). Pure: the GTK timer passes the idle time and acts on the answer.

use std::time::Duration;

/// The idle time after which the bar hides in a window.
pub const HIDE_AFTER: Duration = Duration::from_secs(3);

/// The idle time after which the bar and the pointer hide in fullscreen.
pub const HIDE_AFTER_FULLSCREEN: Duration = Duration::from_millis(2_500);

/// `true` when the bar should hide: it has been idle for long enough and no menu of it is open.
pub fn hidden(idle: Duration, menu_open: bool, fullscreen: bool) -> bool {
    let after = if fullscreen { HIDE_AFTER_FULLSCREEN } else { HIDE_AFTER };
    !menu_open && idle >= after
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_hides_after_three_idle_seconds() {
        assert!(!hidden(Duration::from_millis(2_999), false, false));
        assert!(hidden(Duration::from_secs(3), false, false));
        assert!(hidden(Duration::from_secs(60), false, false));
    }

    #[test]
    fn in_fullscreen_the_bar_and_the_pointer_hide_after_two_and_a_half_seconds() {
        assert!(!hidden(Duration::from_millis(2_499), false, true));
        assert!(hidden(Duration::from_millis(2_500), false, true));
    }

    #[test]
    fn an_open_menu_keeps_the_bar() {
        assert!(!hidden(Duration::from_secs(60), true, false));
        assert!(!hidden(Duration::from_secs(60), true, true));
    }
}
