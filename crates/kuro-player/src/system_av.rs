//! Device volume constants and the mpv volume cap (area 46 `PlayerViewModel.kt` volume code,
//! T-46a-049). The player has no screen brightness control; the mpv video filter `brightness`
//! (T-45a-032) is the only brightness setting.

/// Device volume steps (`maxVolume`).
pub const VOLUME_STEPS: i32 = 15;

/// The mpv property that caps boost: `volume-max` is the boost cap plus 100 (E-46-025). Set it
/// before the first playback and whenever the cap changes.
pub fn volume_max_write(boost_cap: i32) -> (&'static str, i64) {
    ("volume-max", i64::from(boost_cap) + 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_max_is_the_cap_plus_one_hundred() {
        assert_eq!(volume_max_write(30), ("volume-max", 130));
    }
}
