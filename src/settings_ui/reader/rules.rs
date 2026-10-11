//! Enable rules of the reader settings (task T-47a-041). One function decides every dependent row. The split and
//! rotate switches are exclusive; that is a value-write rule, applied in `rows::exclusive_switch`.
use kuro_prefs::reader::ReaderPrefs;

/// Tap-zone mode that turns the zones off (`reader_navigation_mode_*` = 5, "Disabled").
pub const TAP_ZONES_DISABLED: i32 = 5;
/// Scale type "Fit screen" (`pref_image_scale_type_key` = 1).
pub const SCALE_FIT_SCREEN: i32 = 1;

/// Every row whose enabled state depends on another setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    PagerInvertTapZones,
    PagerLandscapeZoom,
    PagerPanWide,
    PagerInvertSplit,
    PagerFlipRotated,
    WebtoonInvertTapZones,
    WebtoonInvertSplit,
    WebtoonFlipRotated,
    FlashDuration,
    FlashInterval,
    FlashColor,
    InvertVolumeKeys,
}

/// The inputs of [`row_enabled`], read from the preferences in one place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReaderState {
    pub pager_tap_zones: i32,
    pub pager_scale_type: i32,
    pub pager_split: bool,
    pub pager_rotate: bool,
    pub webtoon_tap_zones: i32,
    pub webtoon_split: bool,
    pub webtoon_rotate: bool,
    pub flash_on: bool,
    pub volume_keys: bool,
}

impl ReaderState {
    pub fn from_prefs(reader: &ReaderPrefs) -> Self {
        Self {
            pager_tap_zones: reader.navigation_mode_pager.get(),
            pager_scale_type: reader.image_scale_type.get(),
            pager_split: reader.dual_page_split_paged.get(),
            pager_rotate: reader.dual_page_rotate_to_fit.get(),
            webtoon_tap_zones: reader.navigation_mode_webtoon.get(),
            webtoon_split: reader.dual_page_split_webtoon.get(),
            webtoon_rotate: reader.dual_page_rotate_to_fit_webtoon.get(),
            flash_on: reader.flash_on_page_change.get(),
            volume_keys: reader.read_with_volume_keys.get(),
        }
    }
}

/// Whether `row` is enabled (shown) in `state`.
pub fn row_enabled(row: Row, state: &ReaderState) -> bool {
    match row {
        Row::PagerInvertTapZones => state.pager_tap_zones != TAP_ZONES_DISABLED,
        Row::PagerLandscapeZoom => state.pager_scale_type == SCALE_FIT_SCREEN,
        Row::PagerPanWide => state.pager_tap_zones != TAP_ZONES_DISABLED,
        Row::PagerInvertSplit => state.pager_split,
        Row::PagerFlipRotated => state.pager_rotate,
        Row::WebtoonInvertTapZones => state.webtoon_tap_zones != TAP_ZONES_DISABLED,
        Row::WebtoonInvertSplit => state.webtoon_split,
        Row::WebtoonFlipRotated => state.webtoon_rotate,
        Row::FlashDuration | Row::FlashInterval | Row::FlashColor => state.flash_on,
        Row::InvertVolumeKeys => state.volume_keys,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> ReaderState {
        ReaderState {
            pager_tap_zones: 0,
            pager_scale_type: SCALE_FIT_SCREEN,
            pager_split: false,
            pager_rotate: false,
            webtoon_tap_zones: 0,
            webtoon_split: false,
            webtoon_rotate: false,
            flash_on: false,
            volume_keys: false,
        }
    }

    #[test]
    fn paged_invert_tap_zones_need_tap_zones_other_than_disabled() {
        assert!(row_enabled(Row::PagerInvertTapZones, &state()));
        assert!(!row_enabled(Row::PagerInvertTapZones, &ReaderState { pager_tap_zones: 5, ..state() }));
    }

    #[test]
    fn paged_landscape_zoom_needs_fit_screen() {
        assert!(row_enabled(Row::PagerLandscapeZoom, &state()));
        assert!(!row_enabled(Row::PagerLandscapeZoom, &ReaderState { pager_scale_type: 2, ..state() }));
    }

    #[test]
    fn paged_pan_wide_needs_tap_zones_other_than_disabled() {
        assert!(row_enabled(Row::PagerPanWide, &state()));
        assert!(!row_enabled(Row::PagerPanWide, &ReaderState { pager_tap_zones: 5, ..state() }));
    }

    #[test]
    fn paged_invert_split_needs_split_on() {
        assert!(!row_enabled(Row::PagerInvertSplit, &state()));
        assert!(row_enabled(Row::PagerInvertSplit, &ReaderState { pager_split: true, ..state() }));
    }

    #[test]
    fn paged_flip_rotated_needs_rotate_on() {
        assert!(!row_enabled(Row::PagerFlipRotated, &state()));
        assert!(row_enabled(Row::PagerFlipRotated, &ReaderState { pager_rotate: true, ..state() }));
    }

    #[test]
    fn webtoon_invert_tap_zones_use_the_webtoon_mode_not_the_paged_one() {
        let only_webtoon_disabled = ReaderState { webtoon_tap_zones: 5, ..state() };
        assert!(!row_enabled(Row::WebtoonInvertTapZones, &only_webtoon_disabled));
        let only_paged_disabled = ReaderState { pager_tap_zones: 5, ..state() };
        assert!(row_enabled(Row::WebtoonInvertTapZones, &only_paged_disabled));
    }

    #[test]
    fn webtoon_split_and_rotate_rows_follow_their_own_switches() {
        assert!(row_enabled(Row::WebtoonInvertSplit, &ReaderState { webtoon_split: true, ..state() }));
        assert!(!row_enabled(Row::WebtoonInvertSplit, &ReaderState { pager_split: true, ..state() }));
        assert!(row_enabled(Row::WebtoonFlipRotated, &ReaderState { webtoon_rotate: true, ..state() }));
        assert!(!row_enabled(Row::WebtoonFlipRotated, &state()));
    }

    #[test]
    fn eink_rows_need_the_flash_switch() {
        for row in [Row::FlashDuration, Row::FlashInterval, Row::FlashColor] {
            assert!(!row_enabled(row, &state()));
            assert!(row_enabled(row, &ReaderState { flash_on: true, ..state() }));
        }
    }

    #[test]
    fn inverting_volume_keys_needs_volume_keys_on() {
        assert!(!row_enabled(Row::InvertVolumeKeys, &state()));
        assert!(row_enabled(Row::InvertVolumeKeys, &ReaderState { volume_keys: true, ..state() }));
    }
}
