//! Remembered volume (T-45b-053). The stored value `player_volume_value` is the default volume: `-1.0` means nothing is
//! stored. The volume is always remembered (the `pref_remember_volume` switch is cut, its value is hard-coded on): the
//! session start restores a stored value that is not negative, and a change is stored only when it is a real level.
//! Pure: the app reads and writes the preferences.

/// The stored value that means no value was stored.
pub const UNSET: f32 = -1.0;

/// The value to restore at the start of a session, or `None` to leave the level as it is.
pub fn value_to_restore(stored: f32) -> Option<f32> {
    (stored >= 0.0).then_some(stored)
}

/// The value to store after a change, or `None` when the level is not a real one.
pub fn value_to_store(level: f32) -> Option<f32> {
    (level >= 0.0).then_some(level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_45b_053_a_stored_value_is_restored_when_not_negative() {
        assert_eq!(value_to_restore(40.0), Some(40.0));
        assert_eq!(value_to_restore(0.0), Some(0.0));
        assert_eq!(value_to_restore(UNSET), None);
    }

    #[test]
    fn t_45b_053_a_real_level_is_stored() {
        assert_eq!(value_to_store(55.0), Some(55.0));
        assert_eq!(value_to_store(UNSET), None);
    }
}
