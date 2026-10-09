//! Device conditions that gate library updates: Wi-Fi, an unmetered network and the power source (T-08a-028).
//! Linux reads them from NetworkManager and UPower; other platforms report every condition as not met.

/// The three conditions at one moment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeviceState {
    /// The primary connection is Wi-Fi.
    pub on_wifi: bool,
    /// The primary connection is not metered.
    pub network_not_metered: bool,
    /// The device runs on external power.
    pub charging: bool,
}

/// Reads the current conditions. Never fails: a condition that cannot be read is not met.
pub async fn read() -> DeviceState {
    #[cfg(all(target_os = "linux", feature = "platform-linux"))]
    {
        crate::linux::device::read().await
    }
    #[cfg(not(all(target_os = "linux", feature = "platform-linux")))]
    {
        DeviceState::default()
    }
}
