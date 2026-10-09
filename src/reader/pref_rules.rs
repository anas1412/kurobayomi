//! Range rules for numeric reader preferences (T-43b-005, D-7) and the label keys of the tap-inversion enum
//! (T-43b-006). Pure. A stored value outside its range is clamped on read; the stored row is not rewritten.

use std::ops::RangeInclusive;

use kuro_prefs::reader::{ReadingMode, TappingInvertMode, clamp_webtoon_side_padding};

/// `pref_reader_flash_duration`, milliseconds.
pub const FLASH_DURATION_MS: RangeInclusive<i32> = 100..=1500;
/// `pref_reader_flash_interval`, pages.
pub const FLASH_INTERVAL: RangeInclusive<i32> = 1..=10;
/// `custom_brightness_value`. The webtoon side padding (0..=25) is clamped by `kuro_prefs::reader::clamp_webtoon_side_padding`.
pub const BRIGHTNESS: RangeInclusive<i32> = -75..=100;
/// `reader_navigation_mode_pager` and `reader_navigation_mode_webtoon`.
pub const NAVIGATION_MODE: RangeInclusive<i32> = 0..=5;
/// `pref_image_scale_type_key`.
pub const IMAGE_SCALE_TYPE: RangeInclusive<i32> = 1..=6;
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

/// The axis a reading mode pages along (Kotlin `ReadingMode.direction`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadingDirection {
    Horizontal,
    Vertical,
}

/// Horizontal for left-to-right and right-to-left, vertical for the vertical pager, webtoon and continuous vertical,
/// and none for [`ReadingMode::Default`] (T-43b-009).
pub fn reading_direction(mode: ReadingMode) -> Option<ReadingDirection> {
    match mode {
        ReadingMode::Default => None,
        ReadingMode::LeftToRight | ReadingMode::RightToLeft => Some(ReadingDirection::Horizontal),
        ReadingMode::Vertical | ReadingMode::Webtoon | ReadingMode::ContinuousVertical => Some(ReadingDirection::Vertical),
    }
}

/// The label key of a tap-inversion mode (`tapping_inverted_*`).
pub fn tapping_label_key(mode: TappingInvertMode) -> &'static str {
    match mode {
        TappingInvertMode::NONE => "tapping_inverted_none",
        TappingInvertMode::HORIZONTAL => "tapping_inverted_horizontal",
        TappingInvertMode::VERTICAL => "tapping_inverted_vertical",
        TappingInvertMode::BOTH => "tapping_inverted_both",
    }
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
    fn the_other_ranges_clamp_at_their_documented_ends() {
        assert_eq!(clamp_to(0, FLASH_INTERVAL), 1);
        assert_eq!(clamp_to(11, FLASH_INTERVAL), 10);
        assert_eq!(clamp_to(-100, BRIGHTNESS), -75);
        assert_eq!(clamp_to(150, BRIGHTNESS), 100);
        assert_eq!(clamp_to(6, NAVIGATION_MODE), 5);
        assert_eq!(clamp_to(0, IMAGE_SCALE_TYPE), 1);
        assert_eq!(clamp_to(99, IMAGE_SCALE_TYPE), 6);
        assert_eq!(clamp_to(9, ZOOM_START), 4);
        assert_eq!(clamp_to(-1, READER_THEME), 0);
        assert_eq!(clamp_to(7, READER_THEME), 3);
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

    #[test]
    fn e20_each_reading_mode_has_its_direction() {
        use ReadingDirection::{Horizontal, Vertical};
        assert_eq!(reading_direction(ReadingMode::Default), None);
        assert_eq!(reading_direction(ReadingMode::LeftToRight), Some(Horizontal));
        assert_eq!(reading_direction(ReadingMode::RightToLeft), Some(Horizontal));
        assert_eq!(reading_direction(ReadingMode::Vertical), Some(Vertical));
        assert_eq!(reading_direction(ReadingMode::Webtoon), Some(Vertical));
        assert_eq!(reading_direction(ReadingMode::ContinuousVertical), Some(Vertical));
    }

    #[test]
    fn a_value_inside_its_range_is_unchanged() {
        assert_eq!(clamp_to(1, IMAGE_SCALE_TYPE), 1);
        assert_eq!(clamp_to(3, READER_THEME), 3);
        assert_eq!(clamp_to(-75, BRIGHTNESS), -75);
    }

    #[test]
    fn the_tap_inversion_label_keys_follow_the_four_modes() {
        let keys = [
            tapping_label_key(TappingInvertMode::NONE),
            tapping_label_key(TappingInvertMode::HORIZONTAL),
            tapping_label_key(TappingInvertMode::VERTICAL),
            tapping_label_key(TappingInvertMode::BOTH),
        ];
        assert_eq!(keys, ["tapping_inverted_none", "tapping_inverted_horizontal", "tapping_inverted_vertical", "tapping_inverted_both"]);
    }
}
