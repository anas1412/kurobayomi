//! Launcher badge for unread updates (T-01a-025): the count the dock or taskbar shows on the app's icon.
//!
//! The engine is per platform: the Unity launcher entry signal on Linux (`linux::launcher`). Elsewhere there is none
//! yet, so [`publish`] sends nothing.

/// The badge at one moment: the unread count and whether the badge shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Badge {
    pub count: i64,
    pub visible: bool,
}

/// The badge for `unread` updates in the 3-month window. The badge is hidden when the count is 0 (cleared) and
/// while the app lock is on, as the locked widget state does.
pub fn badge(unread: i64) -> Badge {
    Badge { count: unread.max(0), visible: unread > 0 }
}

/// Shows `badge` on the launcher entry of `desktop_file` (`kurobayomi.desktop`). Failures are logged and ignored.
#[allow(unused_variables)]
pub async fn publish(desktop_file: &str, badge: Badge) {
    #[cfg(all(target_os = "linux", feature = "platform-linux"))]
    crate::linux::launcher::publish(desktop_file, badge).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_badge_is_cleared_at_zero_and_hidden_while_locked() {
        assert_eq!(badge(0), Badge { count: 0, visible: false });
        assert_eq!(badge(4), Badge { count: 4, visible: true });
        assert_eq!(badge(-2), Badge { count: 0, visible: false });
    }
}
