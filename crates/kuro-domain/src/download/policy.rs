//! Remove-after-read slot arithmetic (`ReaderViewModel.deleteChapterIfNeeded`, `PlayerViewModel`) and the speed-limit
//! conversion (`MangaDownloader.kt`). Pure functions; the callers hold the lists.

/// The choices of the remove-after-read setting, in the order the settings screen shows them: disabled, last read,
/// second to last, and so on up to fifth to last.
pub const REMOVE_AFTER_READ_OPTIONS: [i32; 6] = [-1, 0, 1, 2, 3, 4];

/// The index of the item to remove, counted back from `current` by `slots`. `-1` is disabled, and so is any other
/// negative value (E-DL-04). Returns `None` when the target is before the first item (E-DL-03).
pub fn remove_after_read_index(current: usize, slots: i32) -> Option<usize> {
    if slots < 0 {
        return None;
    }
    current.checked_sub(slots as usize)
}

/// The item of `sorted` to remove after `current` was read, or `None` (TC-DL-07, TC-DL-08).
pub fn remove_after_read_target<T>(sorted: &[T], current: usize, slots: i32) -> Option<&T> {
    remove_after_read_index(current, slots).and_then(|i| sorted.get(i))
}

/// `downloadOnlyOverWifi`: whether the download job may run on the current network. Kotlin asks WorkManager for an
/// unmetered network; here `network_not_metered` is the platform's answer (`kuro_platform::device`, read by areas 23
/// and 24), so a test can pass a fake.
pub fn download_network_allowed(only_over_wifi: bool, network_not_metered: bool) -> bool {
    !only_over_wifi || network_not_metered
}

/// `download_speed_limit` (KiB/s) as bytes per second. `0` means unthrottled and negative values are treated as `0`
/// (E-DL-02). Widened to `i64` before the multiplication (E-DL-11).
pub fn speed_limit_bytes_per_second(limit_kib: i32) -> i64 {
    i64::from(limit_kib.max(0)) * 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_after_read_selects_current_or_earlier_items() {
        let chapters = ["c1", "c2", "c3", "c4"];
        assert_eq!(remove_after_read_target(&chapters, 3, 0), Some(&"c4"), "0 is the current chapter");
        assert_eq!(remove_after_read_target(&chapters, 3, 1), Some(&"c3"), "1 is the previous one");
        assert_eq!(remove_after_read_target(&chapters, 3, 3), Some(&"c1"));
    }

    #[test]
    fn remove_after_read_does_nothing_when_out_of_range_or_disabled() {
        let chapters = ["c1", "c2", "c3", "c4"];
        assert_eq!(remove_after_read_target(&chapters, 2, 4), None, "E-DL-03: only 2 before the current");
        assert_eq!(remove_after_read_target(&chapters, 3, -1), None, "-1 is disabled");
    }

    #[test]
    fn negative_slots_other_than_minus_one_are_disabled_not_later_items() {
        let chapters = ["c1", "c2", "c3", "c4"];
        assert_eq!(remove_after_read_target(&chapters, 1, -2), None, "E-DL-04: never index+2");
        assert_eq!(remove_after_read_index(1, -7), None);
    }

    #[test]
    fn the_settings_options_are_in_screen_order() {
        assert_eq!(REMOVE_AFTER_READ_OPTIONS, [-1, 0, 1, 2, 3, 4]);
    }

    #[test]
    fn wifi_only_blocks_metered_networks_and_nothing_else() {
        assert!(download_network_allowed(false, false), "any network when the option is off");
        assert!(download_network_allowed(true, true), "unmetered network");
        assert!(!download_network_allowed(true, false), "metered network with wifi-only on");
    }

    #[test]
    fn speed_limit_is_kibibytes_per_second_without_overflow() {
        assert_eq!(speed_limit_bytes_per_second(1), 1024);
        assert_eq!(speed_limit_bytes_per_second(0), 0, "unthrottled");
        assert_eq!(speed_limit_bytes_per_second(-5), 0, "E-DL-02");
        assert_eq!(speed_limit_bytes_per_second(i32::MAX), 2_147_483_647_i64 * 1024, "TC-DL-09");
    }
}
