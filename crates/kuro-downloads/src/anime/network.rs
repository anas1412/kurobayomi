//! Wi-Fi-only policy and network gating (T-24a-031; behaviour 4.2, 4.3; edge cases E-N01, E-N02, E-N04).
//!
//! The policy is a pure function over a [`NetworkState`]. The state comes from a [`NetworkProbe`]. The GUI supplies a
//! NetworkManager probe (`kuro-platform` reads the `State` and `PrimaryConnection` `Type` over D-Bus). [`SysfsNetwork`]
//! is the fallback when NetworkManager is unavailable, and it reads `/sys/class/net/*/operstate` and `wireless`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::downloader::{NO_NETWORK_TEXT, NO_WIFI_TEXT};

/// How long a blocked state must last before the runner stops (E-N04).
pub const DEBOUNCE: Duration = Duration::from_secs(2);

/// The kind of the primary connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    /// NetworkManager `802-11-wireless`.
    Wifi,
    /// NetworkManager `802-3-ethernet`. Allowed under Wi-Fi-only (E-N02).
    Wired,
    /// Anything else, such as a mobile modem or a tunnel. Not Wi-Fi.
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkState {
    Offline,
    Online(Link),
}

/// Reads the current network state. Implemented by the GUI over NetworkManager, or by [`SysfsNetwork`].
pub trait NetworkProbe: Send + Sync {
    fn state(&self) -> NetworkState;
}

/// `None` when downloads may run, else the message for the stop reason (`checkNetworkState`, behaviour 4.2).
pub fn verdict(state: NetworkState, wifi_only: bool) -> Option<&'static str> {
    match state {
        NetworkState::Offline => Some(NO_NETWORK_TEXT),
        NetworkState::Online(Link::Wifi | Link::Wired) => None,
        NetworkState::Online(Link::Other) if wifi_only => Some(NO_WIFI_TEXT),
        NetworkState::Online(Link::Other) => None,
    }
}

/// Fallback probe over sysfs. An interface counts when its `operstate` is `up` and it is not `lo`. A wireless
/// interface has a `wireless` or `phy80211` entry. A non-wireless interface is wired only when its `type` is 1 (Ethernet); other up links are [`Link::Other`]. A wired connection wins over Wi-Fi, and Wi-Fi over other links.
#[derive(Clone, Debug)]
pub struct SysfsNetwork {
    root: PathBuf,
}

impl Default for SysfsNetwork {
    fn default() -> Self {
        Self { root: PathBuf::from("/sys/class/net") }
    }
}

impl SysfsNetwork {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl NetworkProbe for SysfsNetwork {
    fn state(&self) -> NetworkState {
        read_sysfs(&self.root)
    }
}

/// `/sys/class/net/<if>/type` of Ethernet (`ARPHRD_ETHER`). Bridges, tunnels and modems report other values or have none.
const ARPHRD_ETHER: &str = "1";

/// The state of the interfaces under `net_root` (normally `/sys/class/net`).
pub fn read_sysfs(net_root: &Path) -> NetworkState {
    let Ok(entries) = std::fs::read_dir(net_root) else {
        return NetworkState::Offline;
    };
    let (mut wifi, mut wired, mut other) = (false, false, false);
    for entry in entries.flatten() {
        if entry.file_name() == "lo" {
            continue;
        }
        let dir = entry.path();
        let up = std::fs::read_to_string(dir.join("operstate")).is_ok_and(|s| s.trim() == "up");
        if !up {
            continue;
        }
        if dir.join("wireless").exists() || dir.join("phy80211").exists() {
            wifi = true;
        } else if std::fs::read_to_string(dir.join("type")).is_ok_and(|t| t.trim() == ARPHRD_ETHER) {
            wired = true;
        } else {
            other = true;
        }
    }
    match (wired, wifi) {
        (true, _) => NetworkState::Online(Link::Wired),
        (false, true) => NetworkState::Online(Link::Wifi),
        (false, false) if other => NetworkState::Online(Link::Other),
        (false, false) => NetworkState::Offline,
    }
}

/// What the runner should do after an observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateAction {
    /// Stop the run, with this reason as the warning text.
    Stop(&'static str),
    /// The network is back and the run was stopped by the network: start it again (E-N01).
    Resume,
    Nothing,
}

/// Debounces the network verdict. A block must last [`DEBOUNCE`] before the run stops. A run stopped this way starts
/// again when the network comes back, but only if it was running when the network went.
#[derive(Debug, Default)]
pub struct NetworkGate {
    blocked_since: Option<Instant>,
    stopped_by_network: bool,
}

impl NetworkGate {
    /// Feeds one observation. `runner_on` is whether the run is currently on.
    pub fn observe(&mut self, now: Instant, state: NetworkState, wifi_only: bool, runner_on: bool) -> GateAction {
        match verdict(state, wifi_only) {
            Some(reason) => {
                let since = *self.blocked_since.get_or_insert(now);
                if runner_on && now.saturating_duration_since(since) >= DEBOUNCE {
                    self.stopped_by_network = true;
                    return GateAction::Stop(reason);
                }
                GateAction::Nothing
            }
            None => {
                self.blocked_since = None;
                if self.stopped_by_network && !runner_on {
                    self.stopped_by_network = false;
                    return GateAction::Resume;
                }
                GateAction::Nothing
            }
        }
    }

    /// The user paused or cleared the run: a later network return must not start it again.
    pub fn cancel_auto_resume(&mut self) {
        self.stopped_by_network = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(base: Instant, secs: u64) -> Instant {
        base + Duration::from_secs(secs)
    }

    #[test]
    fn verdict_follows_the_linux_mapping() {
        assert_eq!(verdict(NetworkState::Offline, true), Some(NO_NETWORK_TEXT));
        assert_eq!(verdict(NetworkState::Online(Link::Wifi), true), None);
        assert_eq!(verdict(NetworkState::Online(Link::Wired), true), None, "E-N02: wired is allowed under Wi-Fi-only");
        assert_eq!(verdict(NetworkState::Online(Link::Other), true), Some(NO_WIFI_TEXT));
        assert_eq!(verdict(NetworkState::Online(Link::Other), false), None);
    }

    #[test]
    fn a_block_shorter_than_the_debounce_does_not_stop_the_run() {
        let base = Instant::now();
        let mut gate = NetworkGate::default();
        assert_eq!(gate.observe(base, NetworkState::Offline, true, true), GateAction::Nothing);
        assert_eq!(gate.observe(at(base, 1), NetworkState::Offline, true, true), GateAction::Nothing);
        assert_eq!(gate.observe(at(base, 1), NetworkState::Online(Link::Wifi), true, true), GateAction::Nothing);
        assert_eq!(gate.observe(at(base, 2), NetworkState::Offline, true, true), GateAction::Nothing, "flapping restarts the timer");
    }

    #[test]
    fn a_long_block_stops_with_the_reason_and_resumes_on_return() {
        let base = Instant::now();
        let mut gate = NetworkGate::default();
        gate.observe(base, NetworkState::Offline, true, true);
        assert_eq!(gate.observe(at(base, 2), NetworkState::Offline, true, true), GateAction::Stop(NO_NETWORK_TEXT));
        // The caller has stopped the run. Still offline: nothing more to do.
        assert_eq!(gate.observe(at(base, 3), NetworkState::Offline, true, false), GateAction::Nothing);
        assert_eq!(gate.observe(at(base, 9), NetworkState::Online(Link::Wifi), true, false), GateAction::Resume);
        assert_eq!(gate.observe(at(base, 10), NetworkState::Online(Link::Wifi), true, true), GateAction::Nothing);
    }

    #[test]
    fn a_run_that_was_not_running_is_not_resumed_on_return() {
        let base = Instant::now();
        let mut gate = NetworkGate::default();
        gate.observe(base, NetworkState::Offline, true, false);
        assert_eq!(gate.observe(at(base, 5), NetworkState::Offline, true, false), GateAction::Nothing);
        assert_eq!(gate.observe(at(base, 6), NetworkState::Online(Link::Wifi), true, false), GateAction::Nothing);
    }

    #[test]
    fn a_user_pause_cancels_the_pending_resume() {
        let base = Instant::now();
        let mut gate = NetworkGate::default();
        gate.observe(base, NetworkState::Offline, true, true);
        gate.observe(at(base, 2), NetworkState::Offline, true, true);
        gate.cancel_auto_resume();
        assert_eq!(gate.observe(at(base, 9), NetworkState::Online(Link::Wifi), true, false), GateAction::Nothing);
    }

    #[test]
    fn wifi_only_off_lets_a_metered_link_run() {
        let base = Instant::now();
        let mut gate = NetworkGate::default();
        assert_eq!(gate.observe(base, NetworkState::Online(Link::Other), false, true), GateAction::Nothing);
        assert_eq!(gate.observe(at(base, 5), NetworkState::Online(Link::Other), false, true), GateAction::Nothing);
    }

    #[test]
    fn sysfs_reads_up_interfaces_and_skips_loopback() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("lo")).unwrap();
        std::fs::write(root.join("lo/operstate"), "unknown\n").unwrap();
        std::fs::create_dir_all(root.join("wlan0/wireless")).unwrap();
        std::fs::write(root.join("wlan0/operstate"), "up\n").unwrap();
        assert_eq!(read_sysfs(root), NetworkState::Online(Link::Wifi));

        std::fs::create_dir_all(root.join("enp3s0")).unwrap();
        std::fs::write(root.join("enp3s0/operstate"), "up\n").unwrap();
        std::fs::write(root.join("enp3s0/type"), "1\n").unwrap();
        assert_eq!(read_sysfs(root), NetworkState::Online(Link::Wired), "a wired link wins when both are up");

        std::fs::write(root.join("enp3s0/operstate"), "down\n").unwrap();
        std::fs::write(root.join("wlan0/operstate"), "down\n").unwrap();
        assert_eq!(read_sysfs(root), NetworkState::Offline);
    }

    #[test]
    fn a_non_ethernet_link_is_not_wifi_or_wired() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // A modem reports ARPHRD_NONE (65534); a bridge without a type file is not Ethernet either.
        for (name, kind) in [("wwan0", Some("65534\n")), ("docker0", None)] {
            std::fs::create_dir_all(root.join(name)).unwrap();
            std::fs::write(root.join(name).join("operstate"), "up\n").unwrap();
            if let Some(kind) = kind {
                std::fs::write(root.join(name).join("type"), kind).unwrap();
            }
        }
        let state = read_sysfs(root);
        assert_eq!(state, NetworkState::Online(Link::Other));
        assert_eq!(verdict(state, true), Some(NO_WIFI_TEXT));
    }

    #[test]
    fn a_missing_sysfs_root_is_offline() {
        assert_eq!(SysfsNetwork::new("/nonexistent/net").state(), NetworkState::Offline);
    }
}
