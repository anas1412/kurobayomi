//! Device conditions over the system bus: NetworkManager for the primary connection type and its metering,
//! UPower for the power source. A missing bus or service reads as "not met", so a library update waits instead of
//! running on a connection or power source the user did not allow.

use zbus::{Connection, Proxy};

use crate::device::DeviceState;

const NM_DESTINATION: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const UPOWER_DESTINATION: &str = "org.freedesktop.UPower";
const UPOWER_PATH: &str = "/org/freedesktop/UPower";

/// NetworkManager's `PrimaryConnectionType` for a Wi-Fi link.
const WIFI_CONNECTION_TYPE: &str = "802-11-wireless";

/// Reads the three conditions. Each one that cannot be read is `false`.
pub async fn read() -> DeviceState {
    let Ok(conn) = Connection::system().await else {
        return DeviceState::default();
    };
    DeviceState {
        on_wifi: primary_connection_type(&conn).await.as_deref() == Some(WIFI_CONNECTION_TYPE),
        network_not_metered: metered(&conn).await.is_some_and(is_unmetered),
        charging: on_battery(&conn).await == Some(false),
    }
}

async fn primary_connection_type(conn: &Connection) -> Option<String> {
    let proxy = Proxy::new(conn, NM_DESTINATION, NM_PATH, NM_DESTINATION).await.ok()?;
    proxy.get_property::<String>("PrimaryConnectionType").await.ok()
}

async fn metered(conn: &Connection) -> Option<u32> {
    let proxy = Proxy::new(conn, NM_DESTINATION, NM_PATH, NM_DESTINATION).await.ok()?;
    proxy.get_property::<u32>("Metered").await.ok()
}

async fn on_battery(conn: &Connection) -> Option<bool> {
    let proxy = Proxy::new(conn, UPOWER_DESTINATION, UPOWER_PATH, UPOWER_DESTINATION).await.ok()?;
    proxy.get_property::<bool>("OnBattery").await.ok()
}

/// NetworkManager's `NM_METERED_NO` (2) and `NM_METERED_GUESS_NO` (4). Unknown (0) and the metered values count as
/// metered, as Android does when the unmetered capability is absent.
fn is_unmetered(value: u32) -> bool {
    matches!(value, 2 | 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_two_no_metered_values_are_unmetered() {
        assert!(is_unmetered(2));
        assert!(is_unmetered(4));
        for metered in [0, 1, 3, 9] {
            assert!(!is_unmetered(metered), "NM_METERED value {metered} is not unmetered");
        }
    }

    #[test]
    fn the_default_state_meets_no_condition() {
        assert_eq!(DeviceState::default(), DeviceState { on_wifi: false, network_not_metered: false, charging: false });
    }
}
