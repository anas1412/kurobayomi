//! The device conditions an automatic pass waits for (T-26a-027): the device restrictions the user chose (Wi-Fi only,
//! unmetered network, on charger) checked against what the system reports. A value the system cannot give is unknown
//! and never blocks, so a machine without NetworkManager counts as "constraints satisfied" (SC-07).
//!
//! A known "not connected" blocks the pass, as the Kotlin job's `NetworkType.CONNECTED` does. The pure rules are here;
//! the platform reads the facts (`kuro_platform::power_network`).

use std::collections::BTreeSet;

use kuro_prefs::library::{DEVICE_CHARGING, DEVICE_NETWORK_NOT_METERED, DEVICE_ONLY_ON_WIFI};

/// NetworkManager's `PrimaryConnectionType` of a Wi-Fi link.
pub const WIFI_CONNECTION_TYPE: &str = "802-11-wireless";

/// What the pass knows about the device. `None` means the system did not say.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeviceFacts {
    pub connected: Option<bool>,
    /// True when the primary connection is Wi-Fi.
    pub wifi: Option<bool>,
    pub metered: Option<bool>,
    pub on_ac: Option<bool>,
}

/// NetworkManager's `NMMetered` as a metered flag: unknown (0), yes (1) and guess-yes (3) are metered; no (2) and
/// guess-no (4) are not (SC-05).
pub fn nm_metered_is_metered(value: u32) -> bool {
    matches!(value, 0 | 1 | 3)
}

/// The Wi-Fi flag of a NetworkManager `PrimaryConnectionType` text.
pub fn is_wifi_connection(primary_connection_type: &str) -> bool {
    primary_connection_type == WIFI_CONNECTION_TYPE
}

/// True when every restriction the user set holds and the connection is up. Unknown facts never block.
pub fn satisfied(restrictions: &BTreeSet<String>, facts: DeviceFacts) -> bool {
    let blocked = facts.connected == Some(false)
        || (restrictions.contains(DEVICE_ONLY_ON_WIFI) && facts.wifi == Some(false))
        || (restrictions.contains(DEVICE_NETWORK_NOT_METERED) && facts.metered == Some(true))
        || (restrictions.contains(DEVICE_CHARGING) && facts.on_ac == Some(false));
    !blocked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn wifi_only_blocks_when_the_primary_connection_is_not_wireless() {
        // SC-04
        assert!(is_wifi_connection("802-11-wireless"));
        assert!(!is_wifi_connection("802-3-ethernet"));
        let wifi_only = set(&[DEVICE_ONLY_ON_WIFI]);
        let ethernet = DeviceFacts { connected: Some(true), wifi: Some(false), ..DeviceFacts::default() };
        assert!(!satisfied(&wifi_only, ethernet));
        let wireless = DeviceFacts { connected: Some(true), wifi: Some(true), ..DeviceFacts::default() };
        assert!(satisfied(&wifi_only, wireless));
    }

    #[test]
    fn unmetered_only_treats_nm_metered_0_and_3_as_metered() {
        // SC-05: 0 (unknown) and 3 (guess yes) count as metered, as does 1 (yes); 2 and 4 do not.
        assert!(nm_metered_is_metered(0));
        assert!(nm_metered_is_metered(3));
        assert!(nm_metered_is_metered(1));
        assert!(!nm_metered_is_metered(2));
        assert!(!nm_metered_is_metered(4));
        let unmetered = set(&[DEVICE_NETWORK_NOT_METERED]);
        let metered = DeviceFacts { connected: Some(true), metered: Some(nm_metered_is_metered(3)), ..DeviceFacts::default() };
        assert!(!satisfied(&unmetered, metered));
        let free = DeviceFacts { connected: Some(true), metered: Some(nm_metered_is_metered(2)), ..DeviceFacts::default() };
        assert!(satisfied(&unmetered, free));
    }

    #[test]
    fn missing_networkmanager_means_the_constraints_are_satisfied() {
        // SC-07: nothing reported, nothing blocks.
        let all = set(&[DEVICE_ONLY_ON_WIFI, DEVICE_NETWORK_NOT_METERED, DEVICE_CHARGING]);
        assert!(satisfied(&all, DeviceFacts::default()));
    }

    #[test]
    fn charging_and_connection_are_required_when_known() {
        let charging = set(&[DEVICE_CHARGING]);
        assert!(!satisfied(&charging, DeviceFacts { on_ac: Some(false), ..DeviceFacts::default() }));
        assert!(satisfied(&charging, DeviceFacts { on_ac: Some(true), ..DeviceFacts::default() }));
        assert!(!satisfied(&BTreeSet::new(), DeviceFacts { connected: Some(false), ..DeviceFacts::default() }));
    }
}
