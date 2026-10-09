//! Reading mode, orientation and crop-borders state of one manga (`ReadingMode.kt`, `ReaderOrientation.kt`,
//! `SetMangaViewerFlags.kt`, `ReaderViewModel.kt:669-749`). T-43a-004, T-43a-036, T-43a-037.
//!
//! Writes merge under the mask so bits outside the field survive, as `Flag` does upstream. The Linux port does
//! not apply the orientation lock (T-43a-068); the value is still stored and resolved here.

use kuro_prefs::flags::{Flag, with_flag};
use kuro_prefs::reader::{ORIENTATION_MASK, READING_MODE_MASK, ReaderOrientation, ReadingMode};

/// Default reading mode when the preference is unset (`ReaderPreferences.kt:38-41`, right to left).
pub const DEFAULT_READING_MODE_PREF: i32 = 2;
/// Default orientation when the preference is unset (`ReaderPreferences.kt:43-46`, free).
pub const DEFAULT_ORIENTATION_PREF: i32 = 8;

/// The reading mode a manga's viewer flags select. Only the mode bits are read.
pub fn reading_mode_of(viewer_flags: i64) -> ReadingMode {
    ReadingMode::from_preference((viewer_flags & i64::from(READING_MODE_MASK)) as i32)
}

/// `getMangaReadingMode(resolveDefault = true)`: the manga's mode, or the preference when the manga has none.
/// A corrupt global preference that is itself DEFAULT falls back to right-to-left (D-4, E-21), so this never returns
/// [`ReadingMode::Default`].
pub fn resolve_reading_mode(viewer_flags: i64, default_pref: i32) -> ReadingMode {
    match reading_mode_of(viewer_flags) {
        ReadingMode::Default => match ReadingMode::from_preference(default_pref) {
            ReadingMode::Default => ReadingMode::from_preference(DEFAULT_READING_MODE_PREF),
            mode => mode,
        },
        mode => mode,
    }
}

/// Writes a reading mode into the viewer flags, keeping every bit outside [`READING_MODE_MASK`].
pub fn set_reading_mode(viewer_flags: i64, mode: ReadingMode) -> i64 {
    with_flag(viewer_flags, Flag::new(i64::from(mode.flag_value()), i64::from(READING_MODE_MASK)))
}

/// The orientation a manga's viewer flags select. Only the orientation bits are read; unknown codes are `Default`.
pub fn orientation_of(viewer_flags: i64) -> ReaderOrientation {
    ReaderOrientation::from_preference((viewer_flags & i64::from(ORIENTATION_MASK)) as i32)
}

/// `getMangaOrientation(resolveDefault = true)`.
pub fn resolve_orientation(viewer_flags: i64, default_pref: i32) -> ReaderOrientation {
    match orientation_of(viewer_flags) {
        ReaderOrientation::Default => ReaderOrientation::from_preference(default_pref),
        orientation => orientation,
    }
}

/// Writes an orientation into the viewer flags, keeping every bit outside [`ORIENTATION_MASK`].
pub fn set_orientation(viewer_flags: i64, orientation: ReaderOrientation) -> i64 {
    with_flag(viewer_flags, Flag::new(i64::from(orientation.flag_value()), i64::from(ORIENTATION_MASK)))
}

/// Which crop-borders preference a reading mode uses (`ReaderViewModel.toggleCropBorders`, `:742-749`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CropPref {
    Pager,
    Webtoon,
}

/// The crop preference for `mode`: pager modes use `crop_borders`, everything else `crop_borders_webtoon`.
pub fn crop_pref_for(mode: ReadingMode) -> CropPref {
    if ReadingMode::is_pager_type(mode.flag_value()) { CropPref::Pager } else { CropPref::Webtoon }
}

/// The result of flipping the crop toggle: which preference to write and its new value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CropToggle {
    pub pref: CropPref,
    pub value: bool,
}

/// Flips the crop toggle for the current mode. `crop_pager` and `crop_webtoon` are the current values of the two
/// preferences; only the one for the current mode changes.
pub fn toggle_crop(mode: ReadingMode, crop_pager: bool, crop_webtoon: bool) -> CropToggle {
    match crop_pref_for(mode) {
        CropPref::Pager => CropToggle { pref: CropPref::Pager, value: !crop_pager },
        CropPref::Webtoon => CropToggle { pref: CropPref::Webtoon, value: !crop_webtoon },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_mode_values_zero_to_five_round_trip_under_the_mask() {
        for code in 0..=5 {
            let mode = ReadingMode::from_preference(code);
            assert_eq!(mode.flag_value(), code);
            assert_eq!(reading_mode_of(i64::from(code)), mode);
        }
        assert_eq!(READING_MODE_MASK, 0x07);
    }

    #[test]
    fn orientation_values_match_the_flag_table_under_the_mask() {
        let expected = [0x00, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30];
        for (orientation, value) in ReaderOrientation::ALL.into_iter().zip(expected) {
            assert_eq!(orientation.flag_value(), value);
        }
        assert_eq!(ORIENTATION_MASK, 0x38);
    }

    #[test]
    #[allow(non_snake_case)] // the name is the acceptance test name in the plan
    fn reading_mode_fromPreference_unknown_is_default() {
        assert_eq!(ReadingMode::from_preference(0x06), ReadingMode::Default);
        assert_eq!(reading_mode_of(0x06), ReadingMode::Default);
        assert_eq!(ReadingMode::from_preference(0x38), ReadingMode::Default);
        assert_eq!(orientation_of(0x06), ReaderOrientation::Default);
        assert_eq!(orientation_of(0x38), ReaderOrientation::Default);
    }

    #[test]
    fn mask_set_flag_preserves_other_bits() {
        let flags = 0x18 | 0x100 | 0x3;
        let after_mode = set_reading_mode(flags, ReadingMode::Webtoon);
        assert_eq!(after_mode, 0x18 | 0x100 | 0x4);
        let after_orientation = set_orientation(flags, ReaderOrientation::LockedPortrait);
        assert_eq!(after_orientation, 0x20 | 0x100 | 0x3);
        assert_eq!(set_orientation(0x3, ReaderOrientation::Free) & 0x7, 0x3);
    }

    #[test]
    fn manga_mode_zero_resolves_to_preference_and_raw_is_kept_otherwise() {
        assert_eq!(resolve_reading_mode(0, DEFAULT_READING_MODE_PREF), ReadingMode::RightToLeft);
        assert_eq!(resolve_reading_mode(0x3, DEFAULT_READING_MODE_PREF), ReadingMode::Vertical);
        assert_eq!(resolve_orientation(0, DEFAULT_ORIENTATION_PREF), ReaderOrientation::Free);
        assert_eq!(resolve_orientation(0x10, DEFAULT_ORIENTATION_PREF), ReaderOrientation::Portrait);
    }

    #[test]
    fn e21_resolve_never_returns_default_even_with_a_corrupt_global() {
        assert_eq!(resolve_reading_mode(0, 1), ReadingMode::LeftToRight);
        assert_eq!(resolve_reading_mode(0, 0), ReadingMode::RightToLeft, "corrupt global falls back to RTL (D-4)");
        assert_eq!(resolve_reading_mode(0, 9), ReadingMode::RightToLeft);
        assert_ne!(resolve_reading_mode(0, 0), ReadingMode::Default);
    }

    #[test]
    fn e20b_writing_a_mode_keeps_the_orientation_bits() {
        assert_eq!(set_reading_mode(0x38, ReadingMode::RightToLeft), 0x3A);
        assert_eq!(reading_mode_of(set_reading_mode(0x38, ReadingMode::Webtoon) as i64), ReadingMode::Webtoon);
    }

    #[test]
    fn crop_toggle_flips_only_the_preference_for_the_viewer_type() {
        let pager = toggle_crop(ReadingMode::RightToLeft, false, false);
        assert_eq!(pager, CropToggle { pref: CropPref::Pager, value: true });
        let webtoon = toggle_crop(ReadingMode::Webtoon, false, true);
        assert_eq!(webtoon, CropToggle { pref: CropPref::Webtoon, value: false });
        assert_eq!(crop_pref_for(ReadingMode::ContinuousVertical), CropPref::Webtoon);
    }
}
