//! `[reader]` preferences (Aniyomi `ReaderPreferences`) and the reader value tables (`ReadingMode`,
//! `ReaderOrientation`, tap zones, scale types, zoom starts and colour filter modes).
//!
//! Preference values for reading mode and orientation are `Int` codes, so the typed enums here are
//! plain Rust types with `from_preference` for the stored number. Keys marked DROP in the registry
//! (cutout, volume keys, orientation) are kept for backup round trips and have no UI.

use crate::pref_enum;
use crate::registry::define_prefs;

pub const WEBTOON_PADDING_MIN: i32 = 0;
pub const WEBTOON_PADDING_MAX: i32 = 25;
/// Flash duration is stored in units of 100 ms (`pref_reader_flash_duration`).
pub const MILLI_CONVERSION: i32 = 100;

/// The webtoon side padding clamped to its UI range (`WEBTOON_PADDING_MIN..=WEBTOON_PADDING_MAX`).
pub fn clamp_webtoon_side_padding(value: i32) -> i32 {
    value.clamp(WEBTOON_PADDING_MIN, WEBTOON_PADDING_MAX)
}

pref_enum! {
    /// Flash colour of the page-change flash (`ReaderPreferences.FlashColor`).
    pub enum FlashColor { BLACK, WHITE, WHITE_BLACK }
}

pref_enum! {
    /// Which tap directions are inverted (`ReaderPreferences.TappingInvertMode`).
    pub enum TappingInvertMode { NONE, HORIZONTAL, VERTICAL, BOTH }
}

impl TappingInvertMode {
    pub const fn should_invert_horizontal(self) -> bool {
        matches!(self, Self::HORIZONTAL | Self::BOTH)
    }

    pub const fn should_invert_vertical(self) -> bool {
        matches!(self, Self::VERTICAL | Self::BOTH)
    }
}

pref_enum! {
    /// Hide threshold of the reader overlay (`ReaderPreferences.ReaderHideThreshold`).
    pub enum ReaderHideThreshold { HIGHEST, HIGH, LOW, LOWEST }
}

impl ReaderHideThreshold {
    /// The Kotlin `threshold` value.
    pub const fn threshold(self) -> i32 {
        match self {
            Self::HIGHEST => 5,
            Self::HIGH => 13,
            Self::LOW => 31,
            Self::LOWEST => 47,
        }
    }
}

/// `ReadingMode` (`ReadingMode.kt`). The flag value is the stored `pref_default_reading_mode_key` code;
/// the mask is [`READING_MODE_MASK`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadingMode {
    Default,
    LeftToRight,
    RightToLeft,
    Vertical,
    Webtoon,
    ContinuousVertical,
}

pub const READING_MODE_MASK: i32 = 0x7;

/// The viewer family a reading mode opens (Kotlin `ViewerType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewerType {
    Pager,
    Webtoon,
}

/// Returned for [`ReadingMode::Default`]: the preference must be resolved to a concrete mode before a
/// viewer is opened (Kotlin throws `IllegalStateException`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("reading mode DEFAULT must be resolved to the preference default before a viewer is created")]
pub struct UnresolvedReadingMode;

impl ReadingMode {
    pub const ALL: [ReadingMode; 6] = [
        Self::Default,
        Self::LeftToRight,
        Self::RightToLeft,
        Self::Vertical,
        Self::Webtoon,
        Self::ContinuousVertical,
    ];

    pub const fn flag_value(self) -> i32 {
        match self {
            Self::Default => 0x0,
            Self::LeftToRight => 0x1,
            Self::RightToLeft => 0x2,
            Self::Vertical => 0x3,
            Self::Webtoon => 0x4,
            Self::ContinuousVertical => 0x5,
        }
    }

    /// The mode for a stored code. Any unknown code (and `None` in Kotlin) gives [`ReadingMode::Default`].
    pub fn from_preference(code: i32) -> Self {
        Self::ALL.into_iter().find(|m| m.flag_value() == code).unwrap_or(Self::Default)
    }

    /// The viewer family, or [`UnresolvedReadingMode`] for [`ReadingMode::Default`].
    pub fn viewer_type(self) -> Result<ViewerType, UnresolvedReadingMode> {
        match self {
            Self::Default => Err(UnresolvedReadingMode),
            Self::LeftToRight | Self::RightToLeft | Self::Vertical => Ok(ViewerType::Pager),
            Self::Webtoon | Self::ContinuousVertical => Ok(ViewerType::Webtoon),
        }
    }

    /// Whether a stored code opens a pager viewer (LTR, RTL or vertical pager).
    pub fn is_pager_type(code: i32) -> bool {
        Self::from_preference(code).viewer_type() == Ok(ViewerType::Pager)
    }
}

/// `ReaderOrientation` (`ReaderOrientation.kt`). The flag values are the stored
/// `pref_default_orientation_type_key` codes; the mask is [`ORIENTATION_MASK`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderOrientation {
    Default,
    Free,
    Portrait,
    Landscape,
    LockedPortrait,
    LockedLandscape,
    ReversePortrait,
}

pub const ORIENTATION_MASK: i32 = 0x38;

impl ReaderOrientation {
    pub const ALL: [ReaderOrientation; 7] = [
        Self::Default,
        Self::Free,
        Self::Portrait,
        Self::Landscape,
        Self::LockedPortrait,
        Self::LockedLandscape,
        Self::ReversePortrait,
    ];

    pub const fn flag_value(self) -> i32 {
        match self {
            Self::Default => 0x00,
            Self::Free => 0x08,
            Self::Portrait => 0x10,
            Self::Landscape => 0x18,
            Self::LockedPortrait => 0x20,
            Self::LockedLandscape => 0x28,
            Self::ReversePortrait => 0x30,
        }
    }

    /// The orientation for a stored code, [`ReaderOrientation::Default`] when unknown.
    pub fn from_preference(code: i32) -> Self {
        Self::ALL.into_iter().find(|o| o.flag_value() == code).unwrap_or(Self::Default)
    }
}

/// Tap-zone layouts offered in the reader settings, as string-resource keys in Kotlin order
/// (`ReaderPreferences.TapZones`).
pub const TAP_ZONES: [&str; 6] = [
    "label_default",
    "l_nav",
    "kindlish_nav",
    "edge_nav",
    "right_and_left_nav",
    "disabled_nav",
];

/// Image scale types (`ReaderPreferences.ImageScaleType`). The UI stores index plus one in `image_scale_type`.
pub const IMAGE_SCALE_TYPE: [&str; 6] = [
    "scale_type_fit_screen",
    "scale_type_stretch",
    "scale_type_fit_width",
    "scale_type_fit_height",
    "scale_type_original_size",
    "scale_type_smart_fit",
];

/// Zoom start positions (`ReaderPreferences.ZoomStart`). The UI stores index plus one in `zoom_start`.
pub const ZOOM_START: [&str; 4] = [
    "zoom_start_automatic",
    "zoom_start_left",
    "zoom_start_right",
    "zoom_start_center",
];

/// The blend mode a colour filter uses. The GTK side maps each to its compositing operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorFilterBlend {
    SrcOver,
    Modulate,
    Screen,
    Overlay,
    Lighten,
    Darken,
}

/// Colour filter modes in index order (`ReaderPreferences.ColorFilterMode`). All six are offered on Linux.
pub const COLOR_FILTER_MODE: [(&str, ColorFilterBlend); 6] = [
    ("label_default", ColorFilterBlend::SrcOver),
    ("filter_mode_multiply", ColorFilterBlend::Modulate),
    ("filter_mode_screen", ColorFilterBlend::Screen),
    ("filter_mode_overlay", ColorFilterBlend::Overlay),
    ("filter_mode_lighten", ColorFilterBlend::Lighten),
    ("filter_mode_darken", ColorFilterBlend::Darken),
];

define_prefs! {
    /// `[reader]` section: the 54 keys of Aniyomi `ReaderPreferences`.
    pub struct ReaderPrefs in "reader" {
        page_transitions: bool = "pref_enable_transitions_key" => true;
        flash_on_page_change: bool = "pref_reader_flash" => false;
        flash_duration_millis: i32 = "pref_reader_flash_duration" => 100;
        flash_page_interval: i32 = "pref_reader_flash_interval" => 1;
        flash_color: FlashColor = "pref_reader_flash_mode" => FlashColor::BLACK;
        double_tap_anim_speed: i32 = "pref_double_tap_anim_speed" => 500;
        show_page_number: bool = "pref_show_page_number_key" => true;
        show_reading_mode: bool = "pref_show_reading_mode" => true;
        fullscreen: bool = "fullscreen" => true;
        cutout_short: bool = "cutout_short" => true;
        keep_screen_on: bool = "pref_keep_screen_on_key" => true;
        default_reading_mode: i32 = "pref_default_reading_mode_key" => 2;
        default_orientation_type: i32 = "pref_default_orientation_type_key" => 8;
        webtoon_double_tap_zoom_enabled: bool = "pref_enable_double_tap_zoom_webtoon" => true;
        image_scale_type: i32 = "pref_image_scale_type_key" => 1;
        zoom_start: i32 = "pref_zoom_start_key" => 1;
        reader_theme: i32 = "pref_reader_theme_key" => 1;
        always_show_chapter_transition: bool = "always_show_chapter_transition" => true;
        preserve_reading_position: bool = "pref_preserve_reading_position" => false;
        crop_borders: bool = "crop_borders" => false;
        navigate_to_pan: bool = "navigate_pan" => true;
        landscape_zoom: bool = "landscape_zoom" => true;
        crop_borders_webtoon: bool = "crop_borders_webtoon" => false;
        webtoon_side_padding: i32 = "webtoon_side_padding" => 0;
        reader_hide_threshold: ReaderHideThreshold = "reader_hide_threshold" => ReaderHideThreshold::LOW;
        folder_per_manga: bool = "create_folder_per_manga" => false;
        skip_read: bool = "skip_read" => false;
        skip_filtered: bool = "skip_filtered" => true;
        skip_dupe: bool = "skip_dupe" => false;
        webtoon_disable_zoom_out: bool = "webtoon_disable_zoom_out" => false;
        dual_page_split_paged: bool = "pref_dual_page_split" => false;
        dual_page_invert_paged: bool = "pref_dual_page_invert" => false;
        dual_page_split_webtoon: bool = "pref_dual_page_split_webtoon" => false;
        dual_page_invert_webtoon: bool = "pref_dual_page_invert_webtoon" => false;
        dual_page_rotate_to_fit: bool = "pref_dual_page_rotate" => false;
        dual_page_rotate_to_fit_invert: bool = "pref_dual_page_rotate_invert" => false;
        dual_page_rotate_to_fit_webtoon: bool = "pref_dual_page_rotate_webtoon" => false;
        dual_page_rotate_to_fit_invert_webtoon: bool = "pref_dual_page_rotate_invert_webtoon" => false;
        custom_brightness: bool = "pref_custom_brightness_key" => false;
        custom_brightness_value: i32 = "custom_brightness_value" => 0;
        color_filter: bool = "pref_color_filter_key" => false;
        color_filter_value: i32 = "color_filter_value" => 0;
        color_filter_mode: i32 = "color_filter_mode" => 0;
        grayscale: bool = "pref_grayscale" => false;
        inverted_colors: bool = "pref_inverted_colors" => false;
        read_with_long_tap: bool = "reader_long_tap" => true;
        read_with_volume_keys: bool = "reader_volume_keys" => false;
        read_with_volume_keys_inverted: bool = "reader_volume_keys_inverted" => false;
        navigation_mode_pager: i32 = "reader_navigation_mode_pager" => 0;
        navigation_mode_webtoon: i32 = "reader_navigation_mode_webtoon" => 0;
        pager_nav_inverted: TappingInvertMode = "reader_tapping_inverted" => TappingInvertMode::NONE;
        webtoon_nav_inverted: TappingInvertMode = "reader_tapping_inverted_webtoon" => TappingInvertMode::NONE;
        show_navigation_overlay_new_user: bool = "reader_navigation_overlay_new_user" => true;
        show_navigation_overlay_on_start: bool = "reader_navigation_overlay_on_start" => false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{REGISTRY, lookup};
    use crate::store::PrefStore;
    use crate::testutil::assert_section_registered;
    use crate::values::{Decoded, PrefType, PrefValue};

    #[test]
    fn the_section_holds_every_reader_row() {
        assert_eq!(ReaderPrefs::FIELDS.len(), 54);
        let reader = ReaderPrefs::new(PrefStore::in_memory());
        assert_section_registered(ReaderPrefs::TABLE, ReaderPrefs::FIELDS, &reader.defaults());
    }

    #[test]
    fn plan_defaults_hold_on_an_empty_store() {
        let r = ReaderPrefs::new(PrefStore::in_memory());
        assert!(r.page_transitions.get());
        assert_eq!(r.flash_duration_millis.get(), 100);
        assert_eq!(r.double_tap_anim_speed.get(), 500);
        assert_eq!(r.default_reading_mode.get(), 2);
        assert_eq!(r.default_orientation_type.get(), 8);
        assert_eq!(r.image_scale_type.get(), 1);
        assert_eq!(r.zoom_start.get(), 1);
        assert_eq!(r.reader_theme.get(), 1);
        assert_eq!(r.webtoon_side_padding.get(), 0);
        assert_eq!(r.reader_hide_threshold.get(), ReaderHideThreshold::LOW);
        assert_eq!(r.flash_color.get(), FlashColor::BLACK);
        assert_eq!(r.pager_nav_inverted.get(), TappingInvertMode::NONE);
    }

    #[test]
    fn drop_keys_are_registered_for_round_trip() {
        for key in ["cutout_short", "reader_volume_keys", "reader_volume_keys_inverted", "pref_default_orientation_type_key"] {
            assert!(lookup(key).is_some(), "{key}");
        }
    }

    #[test]
    fn reading_mode_codes_and_pager_check() {
        for (code, mode) in [
            (0, ReadingMode::Default),
            (1, ReadingMode::LeftToRight),
            (2, ReadingMode::RightToLeft),
            (3, ReadingMode::Vertical),
            (4, ReadingMode::Webtoon),
            (5, ReadingMode::ContinuousVertical),
        ] {
            assert_eq!(ReadingMode::from_preference(code), mode);
            assert_eq!(mode.flag_value(), code);
        }
        assert_eq!(ReadingMode::from_preference(9), ReadingMode::Default);
        assert_eq!(ReadingMode::from_preference(-1), ReadingMode::Default);
        assert!(ReadingMode::is_pager_type(1) && ReadingMode::is_pager_type(2) && ReadingMode::is_pager_type(3));
        assert!(!ReadingMode::is_pager_type(4) && !ReadingMode::is_pager_type(5));
        assert_eq!(ReadingMode::Default.viewer_type(), Err(UnresolvedReadingMode));
        assert_eq!(ReadingMode::ContinuousVertical.viewer_type(), Ok(ViewerType::Webtoon));
        assert_eq!(READING_MODE_MASK, 0x7);
    }

    #[test]
    fn orientation_codes_match_the_flag_table() {
        let codes: Vec<i32> = ReaderOrientation::ALL.iter().map(|o| o.flag_value()).collect();
        assert_eq!(codes, [0x00, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30]);
        assert_eq!(ReaderOrientation::from_preference(0x18), ReaderOrientation::Landscape);
        assert_eq!(ReaderOrientation::from_preference(0x7), ReaderOrientation::Default);
        assert_eq!(ORIENTATION_MASK, 0x38);
    }

    #[test]
    fn value_tables_have_the_documented_lengths_and_order() {
        assert_eq!(TAP_ZONES.len(), 6);
        assert_eq!(TAP_ZONES[1], "l_nav");
        assert_eq!(IMAGE_SCALE_TYPE.len(), 6);
        assert_eq!(IMAGE_SCALE_TYPE[5], "scale_type_smart_fit");
        assert_eq!(ZOOM_START.len(), 4);
        assert_eq!(ZOOM_START[3], "zoom_start_center");
        assert_eq!(COLOR_FILTER_MODE.len(), 6);
        let blends: Vec<ColorFilterBlend> = COLOR_FILTER_MODE.iter().map(|(_, b)| *b).collect();
        assert_eq!(
            blends,
            [
                ColorFilterBlend::SrcOver,
                ColorFilterBlend::Modulate,
                ColorFilterBlend::Screen,
                ColorFilterBlend::Overlay,
                ColorFilterBlend::Lighten,
                ColorFilterBlend::Darken,
            ]
        );
    }

    #[test]
    fn hide_threshold_and_tap_inversion_values() {
        let thresholds: Vec<i32> = [
            ReaderHideThreshold::HIGHEST,
            ReaderHideThreshold::HIGH,
            ReaderHideThreshold::LOW,
            ReaderHideThreshold::LOWEST,
        ]
        .iter()
        .map(|t| t.threshold())
        .collect();
        assert_eq!(thresholds, [5, 13, 31, 47]);
        assert!(TappingInvertMode::BOTH.should_invert_horizontal() && TappingInvertMode::BOTH.should_invert_vertical());
        assert!(TappingInvertMode::HORIZONTAL.should_invert_horizontal());
        assert!(!TappingInvertMode::HORIZONTAL.should_invert_vertical());
        assert!(!TappingInvertMode::NONE.should_invert_horizontal());
    }

    #[test]
    fn webtoon_padding_is_clamped_to_its_range() {
        assert_eq!(clamp_webtoon_side_padding(-4), 0);
        assert_eq!(clamp_webtoon_side_padding(30), 25);
        assert_eq!(clamp_webtoon_side_padding(12), 12);
        assert_eq!((WEBTOON_PADDING_MIN, WEBTOON_PADDING_MAX, MILLI_CONVERSION), (0, 25, 100));
    }

    #[test]
    fn enum_values_are_stored_by_their_kotlin_names() {
        assert_eq!(FlashColor::WHITE_BLACK.encode(), PrefValue::Str("WHITE_BLACK".into()));
        assert_eq!(
            TappingInvertMode::decode(&PrefValue::Str("VERTICAL".into())),
            Decoded::Ok(TappingInvertMode::VERTICAL)
        );
        assert!(REGISTRY.iter().any(|m| m.key == "reader_tapping_inverted_webtoon"));
    }
}
