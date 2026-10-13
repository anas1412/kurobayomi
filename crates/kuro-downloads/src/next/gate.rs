//! The network gate, one for both lanes (docs/design/unified-downloads.md 2.6). The two-second debounce of the old anime
//! `NetworkGate`, applied per lane. The app feeds [`Connectivity`] from the system network monitor.

use std::time::{Duration, Instant};

use kuro_platform::kind::Kind;

use crate::downloader::{NO_NETWORK_TEXT, NO_WIFI_TEXT};

/// How long a blocked state must last before a lane stops.
pub const DEBOUNCE: Duration = Duration::from_secs(2);

/// Network state as the app reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Connectivity {
    pub online: bool,
    pub metered: bool,
}

/// What a lane should do after an observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateAction {
    /// Stop the lane, with this text as the warning.
    Stop(&'static str),
    /// The network is back and the network had stopped this lane: start it again.
    Resume,
    Nothing,
}

/// `None` when downloads may run, else the warning text: offline stops with the network text, and a metered link stops
/// with the Wi-Fi text when "Wi-Fi only" is on.
pub fn verdict(c: Connectivity, only_over_wifi: bool) -> Option<&'static str> {
    if !c.online {
        Some(NO_NETWORK_TEXT)
    } else if c.metered && only_over_wifi {
        Some(NO_WIFI_TEXT)
    } else {
        None
    }
}

/// A lane stopped by the network starts again by itself when the network is back, unless the user paused it meanwhile.
/// Lane order is `Kind::ALL`: manga, then anime.
#[derive(Debug, Default)]
pub struct Gate {
    blocked_since: Option<Instant>,
    stopped_by_network: [bool; 2],
}

fn slot(kind: Kind) -> usize {
    match kind {
        Kind::Manga => 0,
        Kind::Anime => 1,
    }
}

impl Gate {
    /// Feeds one observation. `running` says which lanes are running now. A block must last [`DEBOUNCE`] before a running
    /// lane stops, so a flapping link does not.
    pub fn observe(&mut self, now: Instant, c: Connectivity, only_over_wifi: bool, running: [bool; 2]) -> [GateAction; 2] {
        let mut out = [GateAction::Nothing; 2];
        match verdict(c, only_over_wifi) {
            Some(reason) => {
                let since = *self.blocked_since.get_or_insert(now);
                if now.saturating_duration_since(since) >= DEBOUNCE {
                    for i in 0..2 {
                        if running[i] {
                            self.stopped_by_network[i] = true;
                            out[i] = GateAction::Stop(reason);
                        }
                    }
                }
            }
            None => {
                self.blocked_since = None;
                for i in 0..2 {
                    if self.stopped_by_network[i] && !running[i] {
                        self.stopped_by_network[i] = false;
                        out[i] = GateAction::Resume;
                    }
                }
            }
        }
        out
    }

    /// The user paused or cleared the lane: a later network return must not start it again.
    pub fn user_paused(&mut self, kind: Kind) {
        self.stopped_by_network[slot(kind)] = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFFLINE: Connectivity = Connectivity { online: false, metered: false };
    const METERED: Connectivity = Connectivity { online: true, metered: true };
    const FREE: Connectivity = Connectivity { online: true, metered: false };

    fn at(base: Instant, secs: u64) -> Instant {
        base + Duration::from_secs(secs)
    }

    #[test]
    fn verdict_follows_the_connectivity_table() {
        assert_eq!(verdict(OFFLINE, true), Some(NO_NETWORK_TEXT));
        assert_eq!(verdict(OFFLINE, false), Some(NO_NETWORK_TEXT));
        assert_eq!(verdict(METERED, true), Some(NO_WIFI_TEXT));
        assert_eq!(verdict(METERED, false), None);
        assert_eq!(verdict(FREE, true), None);
    }

    #[test]
    fn a_block_shorter_than_the_debounce_stops_nothing() {
        let base = Instant::now();
        let mut gate = Gate::default();
        assert_eq!(gate.observe(base, OFFLINE, false, [true, true]), [GateAction::Nothing; 2]);
        assert_eq!(gate.observe(at(base, 1), OFFLINE, false, [true, true]), [GateAction::Nothing; 2]);
        assert_eq!(gate.observe(at(base, 1), FREE, false, [true, true]), [GateAction::Nothing; 2]);
        assert_eq!(gate.observe(at(base, 2), OFFLINE, false, [true, true]), [GateAction::Nothing; 2], "flapping restarts the timer");
    }

    #[test]
    fn a_long_block_stops_both_running_lanes_and_the_return_resumes_them() {
        let base = Instant::now();
        let mut gate = Gate::default();
        gate.observe(base, OFFLINE, false, [true, true]);
        let stop = GateAction::Stop(NO_NETWORK_TEXT);
        assert_eq!(gate.observe(at(base, 2), OFFLINE, false, [true, true]), [stop, stop]);
        // The caller stopped both lanes. Still offline: nothing more to do.
        assert_eq!(gate.observe(at(base, 3), OFFLINE, false, [false, false]), [GateAction::Nothing; 2]);
        assert_eq!(gate.observe(at(base, 9), FREE, false, [false, false]), [GateAction::Resume; 2]);
        assert_eq!(gate.observe(at(base, 10), FREE, false, [true, true]), [GateAction::Nothing; 2]);
    }

    #[test]
    fn a_lane_the_user_paused_stays_paused_when_the_network_returns() {
        let base = Instant::now();
        let mut gate = Gate::default();
        gate.observe(base, OFFLINE, false, [true, true]);
        gate.observe(at(base, 2), OFFLINE, false, [true, true]);
        gate.user_paused(Kind::Anime);
        assert_eq!(gate.observe(at(base, 5), FREE, false, [false, false]), [GateAction::Resume, GateAction::Nothing]);
    }

    #[test]
    fn a_lane_that_was_not_running_is_not_stopped_or_resumed() {
        let base = Instant::now();
        let mut gate = Gate::default();
        gate.observe(base, METERED, true, [false, true]);
        let out = gate.observe(at(base, 3), METERED, true, [false, true]);
        assert_eq!(out, [GateAction::Nothing, GateAction::Stop(NO_WIFI_TEXT)]);
        assert_eq!(gate.observe(at(base, 4), FREE, true, [false, false]), [GateAction::Nothing, GateAction::Resume]);
    }
}
