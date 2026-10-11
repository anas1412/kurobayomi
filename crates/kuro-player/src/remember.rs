//! Remembered volume (T-45b-053; `pref_remember_volume`). The stored value `player_volume_value` defaults to
//! `-1.0`, which means nothing is stored. The session start restores the value only when its switch is on and the
//! value is not negative; a change is stored only under the same conditions. Pure: the app reads and writes the preferences.

/// The stored value that means no value was stored.
pub const UNSET: f32 = -1.0;

/// The value to restore at the start of a session, or `None` to leave the level as it is.
pub fn value_to_restore(remember: bool, stored: f32) -> Option<f32> {
    (remember && stored >= 0.0).then_some(stored)
}

/// The value to store after a change, or `None` when the switch is off or the level is not a real one.
pub fn value_to_store(remember: bool, level: f32) -> Option<f32> {
    (remember && level >= 0.0).then_some(level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_45b_053_a_value_is_restored_only_when_on_and_not_negative() {
        assert_eq!(value_to_restore(true, 40.0), Some(40.0));
        assert_eq!(value_to_restore(true, 0.0), Some(0.0));
        assert_eq!(value_to_restore(true, UNSET), None);
        assert_eq!(value_to_restore(false, 40.0), None);
    }

    #[test]
    fn t_45b_053_a_change_is_stored_only_when_on() {
        assert_eq!(value_to_store(true, 55.0), Some(55.0));
        assert_eq!(value_to_store(false, 55.0), None);
        assert_eq!(value_to_store(true, UNSET), None);
    }
}
