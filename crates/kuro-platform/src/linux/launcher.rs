//! Launcher badge for unread updates (T-01a-025). The count goes out as the `com.canonical.Unity.LauncherEntry`
//! `Update` signal, which docks and taskbars read for the badge. Without a known desktop file name nothing is sent.

use std::collections::HashMap;

use zbus::zvariant::Value;
use zbus::Connection;

use crate::launcher::Badge;

const INTERFACE: &str = "com.canonical.Unity.LauncherEntry";
const SIGNAL: &str = "Update";

/// The `application://` URI of a desktop file name (`kurobayomi.desktop`).
pub fn app_uri(desktop_file: &str) -> String {
    format!("application://{desktop_file}")
}

/// The signal properties for a badge: `count` and `count-visible`.
pub fn properties(badge: Badge) -> HashMap<&'static str, Value<'static>> {
    HashMap::from([("count", Value::from(badge.count)), ("count-visible", Value::from(badge.visible))])
}

/// Publishes the badge on the session bus. A failure to reach the bus is logged at DEBUG and otherwise ignored.
pub async fn publish(desktop_file: &str, badge: Badge) {
    let Ok(conn) = Connection::session().await else {
        tracing::debug!("launcher badge: no session bus");
        return;
    };
    let body = (app_uri(desktop_file), properties(badge));
    let sent = conn
        .emit_signal(None::<&str>, "/", INTERFACE, SIGNAL, &body)
        .await;
    if let Err(err) = sent {
        tracing::debug!(%err, "launcher badge signal not sent");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uri_and_the_properties_carry_the_count() {
        assert_eq!(app_uri("kurobayomi.desktop"), "application://kurobayomi.desktop");
        let props = properties(Badge { count: 3, visible: true });
        assert_eq!(props.get("count"), Some(&Value::from(3_i64)));
        assert_eq!(props.get("count-visible"), Some(&Value::from(true)));
    }
}
