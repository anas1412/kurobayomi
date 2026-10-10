//! Reads [`Conditions`] on Linux: NetworkManager for the connection and its metering, UPower for the power source and
//! the battery warning level. A machine without NetworkManager on the bus gets its network part from the GIO network
//! monitor. A missing UPower reads as external power with a healthy battery, which is what a desktop reports.

use gio::prelude::*;
use zbus::{Connection, Proxy};

use crate::power_network::Conditions;

const NM_DESTINATION: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const UPOWER_DESTINATION: &str = "org.freedesktop.UPower";
const UPOWER_PATH: &str = "/org/freedesktop/UPower";
const UPOWER_DISPLAY_DEVICE: &str = "/org/freedesktop/UPower/devices/DisplayDevice";

/// NetworkManager `State` of a connection that reaches at least the local site (`NM_STATE_CONNECTED_SITE`).
const NM_STATE_CONNECTED_SITE: u32 = 60;
/// UPower `WarningLevel` from which the battery counts as low: 3 low, 4 critical, 5 action.
const UPOWER_WARNING_LOW: u32 = 3;

pub async fn read() -> Conditions {
    let conn = Connection::system().await.ok();
    let (connected, metered) = match &conn {
        Some(conn) => match network_manager(conn).await {
            Some(reading) => reading,
            None => gio_network(),
        },
        None => gio_network(),
    };
    let (on_ac, battery_low) = match &conn {
        Some(conn) => (on_battery(conn).await != Some(true), battery_warning(conn).await.is_some_and(is_low)),
        None => (true, false),
    };
    Conditions { connected, metered, on_ac, battery_low }
}

/// `(connected, metered)` from NetworkManager, or `None` when it is not on the bus.
async fn network_manager(conn: &Connection) -> Option<(bool, bool)> {
    let proxy = Proxy::new(conn, NM_DESTINATION, NM_PATH, NM_DESTINATION).await.ok()?;
    let state = proxy.get_property::<u32>("State").await.ok()?;
    // An unreadable `Metered` counts as metered, as Android does when the unmetered capability is absent.
    let metered = proxy.get_property::<u32>("Metered").await.map_or(true, is_metered);
    Some((is_connected(state), metered))
}

fn gio_network() -> (bool, bool) {
    let monitor = gio::NetworkMonitor::default();
    (monitor.is_network_available(), monitor.is_network_metered())
}

async fn on_battery(conn: &Connection) -> Option<bool> {
    let proxy = Proxy::new(conn, UPOWER_DESTINATION, UPOWER_PATH, UPOWER_DESTINATION).await.ok()?;
    proxy.get_property::<bool>("OnBattery").await.ok()
}

async fn battery_warning(conn: &Connection) -> Option<u32> {
    let proxy =
        Proxy::new(conn, UPOWER_DESTINATION, UPOWER_DISPLAY_DEVICE, "org.freedesktop.UPower.Device").await.ok()?;
    proxy.get_property::<u32>("WarningLevel").await.ok()
}

fn is_connected(state: u32) -> bool {
    state >= NM_STATE_CONNECTED_SITE
}

/// NetworkManager's `NM_METERED_NO` (2) and `NM_METERED_GUESS_NO` (4) are unmetered. Unknown (0) and the metered
/// values count as metered.
fn is_metered(value: u32) -> bool {
    !matches!(value, 2 | 4)
}

fn is_low(level: u32) -> bool {
    level >= UPOWER_WARNING_LOW
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_site_or_global_connection_counts_as_connected() {
        for state in [0, 10, 20, 30, 40, 50] {
            assert!(!is_connected(state), "NM state {state}");
        }
        assert!(is_connected(60));
        assert!(is_connected(70));
    }

    #[test]
    fn only_the_two_no_metered_values_are_unmetered() {
        assert!(!is_metered(2) && !is_metered(4));
        for value in [0, 1, 3, 9] {
            assert!(is_metered(value), "NM_METERED value {value}");
        }
    }

    #[test]
    fn the_battery_is_low_from_warning_level_three() {
        assert!(!is_low(0) && !is_low(1) && !is_low(2));
        assert!(is_low(3) && is_low(4) && is_low(5));
    }
}
