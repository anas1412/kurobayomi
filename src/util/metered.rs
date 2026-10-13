//! Metered-connection policy (Kotlin `NetworkStateTracker`, T-53a-025).
//!
//! `is_metered` comes from the GIO network monitor and from the NetworkManager `Metered` value when the platform layer
//! can read it. NetworkManager's values: 0 unknown, 1 yes, 2 no, 3 guess yes, 4 guess no; 1 and 3 mean metered.
//! An unknown state changes no behaviour: a job that waits for an unmetered connection waits only on a known metered one.

#[cfg(test)]
/// What is known about the connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metered {
    Yes,
    No,
    Unknown,
}

#[cfg(test)]
/// The policy text for the settings help of the "only on unmetered networks" rows.
pub const POLICY_HELP: &str = "A connection that the desktop or NetworkManager marks as metered pauses these jobs. \
When the connection state is not known, nothing changes.";

#[cfg(test)]
/// NetworkManager's `Metered` property as a state.
pub fn from_network_manager(value: u32) -> Metered {
    match value {
        1 | 3 => Metered::Yes,
        2 | 4 => Metered::No,
        _ => Metered::Unknown,
    }
}

#[cfg(test)]
/// Joins the two readings. A metered reading from either source wins. `gio` is `None` when no network monitor answers,
/// `network_manager` is `None` when NetworkManager is absent or the property is unknown.
pub fn combine(gio: Option<bool>, network_manager: Option<u32>) -> Metered {
    let nm = network_manager.map_or(Metered::Unknown, from_network_manager);
    match (gio, nm) {
        (Some(true), _) | (_, Metered::Yes) => Metered::Yes,
        (Some(false), _) | (None, Metered::No) => Metered::No,
        (None, Metered::Unknown) => Metered::Unknown,
    }
}

#[cfg(test)]
/// `is_metered` of the network state: true only for a known metered connection.
pub fn is_metered(gio: Option<bool>, network_manager: Option<u32>) -> bool {
    combine(gio, network_manager) == Metered::Yes
}

#[cfg(test)]
/// Whether a job set to run only on an unmetered connection must wait now.
pub fn must_wait(only_unmetered: bool, state: Metered) -> bool {
    only_unmetered && state == Metered::Yes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_manager_values_one_and_three_are_metered() {
        assert_eq!(from_network_manager(1), Metered::Yes);
        assert_eq!(from_network_manager(3), Metered::Yes);
        assert_eq!(from_network_manager(2), Metered::No);
        assert_eq!(from_network_manager(4), Metered::No);
        assert_eq!(from_network_manager(0), Metered::Unknown);
        assert_eq!(from_network_manager(99), Metered::Unknown);
    }

    #[test]
    fn either_source_saying_metered_wins() {
        assert_eq!(combine(Some(true), Some(2)), Metered::Yes);
        assert_eq!(combine(Some(false), Some(3)), Metered::Yes);
        assert!(is_metered(Some(true), None));
        assert!(is_metered(None, Some(1)));
    }

    #[test]
    fn a_known_unmetered_reading_is_not_metered() {
        assert_eq!(combine(Some(false), None), Metered::No);
        assert_eq!(combine(Some(false), Some(0)), Metered::No);
        assert_eq!(combine(None, Some(4)), Metered::No);
        assert!(!is_metered(Some(false), Some(2)));
    }

    #[test]
    fn nothing_known_is_unknown_and_not_metered() {
        assert_eq!(combine(None, None), Metered::Unknown);
        assert_eq!(combine(None, Some(0)), Metered::Unknown);
        assert!(!is_metered(None, None));
    }

    #[test]
    fn an_unknown_state_never_makes_a_job_wait() {
        assert!(must_wait(true, Metered::Yes));
        assert!(!must_wait(true, Metered::Unknown));
        assert!(!must_wait(true, Metered::No));
        assert!(!must_wait(false, Metered::Yes));
    }

    #[test]
    fn the_help_text_states_the_unknown_rule() {
        assert!(POLICY_HELP.contains("metered"));
        assert!(POLICY_HELP.contains("not known"));
    }
}
