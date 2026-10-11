//! Range rules for numeric reader preferences (T-43b-005, D-7) and the label keys of the tap-inversion enum
//! (T-43b-006). Pure. A stored value outside its range is clamped on read; the stored row is not rewritten.

use std::ops::RangeInclusive;

use kuro_prefs::reader::{ReadingMode, TappingInvertMode, clamp_webtoon_side_padding};

/// `pref_reader_flash_duration`, milliseconds.
pub const FLASH_DURATION_MS: RangeInclusive<i32> = 100..=1500;
/// `pref_reader_flash_interval`, pages.
pub const FLASH_INTERVAL: RangeInclusive<i32> = 1..=10;
/// `reader_navigation_mode_pager` and `reader_navigation_mode_webtoon`.
pub const NAVIGATION_MODE: RangeInclusive<i32> = 0..=5;
/// `pref_zoom_start_key`.
pub const ZOOM_START: RangeInclusive<i32> = 1..=4;
/// `pref_reader_theme_key`.
pub const READER_THEME: RangeInclusive<i32> = 0..=3;

/// Clamps a stored value into its range. Never panics: the bounds are constants with `lo <= hi`.
pub fn clamp_to(value: i32, range: RangeInclusive<i32>) -> i32 {
    value.max(*range.start()).min(*range.end())
}

/// The flash duration to use for a stored value: clamped to 100..=1500 ms and rounded to the nearest multiple of
/// 100 (the stored unit, `MILLI_CONVERSION`).
pub fn flash_duration_ms(stored: i32) -> i32 {
    let step = kuro_prefs::reader::MILLI_CONVERSION;
    let rounded = (clamp_to(stored, FLASH_DURATION_MS) + step / 2) / step * step;
    clamp_to(rounded, FLASH_DURATION_MS)
}

/// The flash interval to use for a stored value, clamped to 1..=10 pages.
pub fn flash_interval(stored: i32) -> i32 {
    clamp_to(stored, FLASH_INTERVAL)
}

/// The webtoon side padding to use for a stored value, clamped to 0..=25 (D-7).
pub fn side_padding(stored: i32) -> i32 {
    clamp_webtoon_side_padding(stored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flash_duration_is_clamped_to_one_hundred_to_fifteen_hundred_milliseconds() {
        assert_eq!(clamp_to(50, FLASH_DURATION_MS), 100);
        assert_eq!(clamp_to(2000, FLASH_DURATION_MS), 1500);
        assert_eq!(clamp_to(400, FLASH_DURATION_MS), 400);
    }

    #[test]
    fn flash_duration_is_a_multiple_of_one_hundred_inside_the_range() {
        assert_eq!(flash_duration_ms(50), 100);
        assert_eq!(flash_duration_ms(2000), 1500);
        assert_eq!(flash_duration_ms(449), 400);
        assert_eq!(flash_duration_ms(450), 500);
        assert_eq!(flash_duration_ms(1500), 1500);
        assert_eq!(flash_duration_ms(i32::MAX), 1500);
    }

    #[test]
    fn interval_and_side_padding_clamp_on_read() {
        assert_eq!((flash_interval(0), flash_interval(11), flash_interval(4)), (1, 10, 4));
        assert_eq!((side_padding(30), side_padding(-2), side_padding(12)), (25, 0, 12));
    }

}
