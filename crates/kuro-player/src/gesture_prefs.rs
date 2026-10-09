//! Gesture preference catalogue (area 48 `GesturePreferences.kt`, data doc section 4.4, T-45b-047).
//! Thirteen keys with their Kotlin defaults. Enum values are stored by their Kotlin names, so an
//! unknown name falls back to the default (`getEnum` semantics).

use crate::enums::SingleActionGesture;

/// The preference keys, in the order of the data doc.
pub const KEYS: [&str; 13] = [
    "pref_gesture_volume_brightness",
    "pref_swap_volume_and_brightness",
    "pref_gesture_horizontal_seek",
    "pref_show_seekbar",
    "pref_default_intro_length",
    "pref_skip_length_preference",
    "pref_player_smooth_seek",
    "pref_left_double_tap",
    "pref_center_double_tap",
    "pref_right_double_tap",
    "pref_media_previous",
    "pref_media_playpause",
    "pref_media_next",
];

/// Typed gesture settings with the store defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GesturePrefs {
    pub gesture_volume_brightness: bool,
    pub swap_volume_and_brightness: bool,
    pub gesture_horizontal_seek: bool,
    pub show_seekbar: bool,
    pub default_intro_length: i64,
    pub skip_length: i64,
    pub smooth_seek: bool,
    pub left_double_tap: SingleActionGesture,
    pub center_double_tap: SingleActionGesture,
    pub right_double_tap: SingleActionGesture,
    pub media_previous: SingleActionGesture,
    pub media_playpause: SingleActionGesture,
    pub media_next: SingleActionGesture,
}

impl Default for GesturePrefs {
    fn default() -> Self {
        Self {
            gesture_volume_brightness: true,
            swap_volume_and_brightness: false,
            gesture_horizontal_seek: true,
            show_seekbar: false,
            default_intro_length: 85,
            skip_length: 10,
            smooth_seek: false,
            left_double_tap: SingleActionGesture::Seek,
            center_double_tap: SingleActionGesture::PlayPause,
            right_double_tap: SingleActionGesture::Seek,
            media_previous: SingleActionGesture::Switch,
            media_playpause: SingleActionGesture::PlayPause,
            media_next: SingleActionGesture::Switch,
        }
    }
}

/// Reads an action stored by its Kotlin enum name. Anything else gives `default`.
pub fn action_from_name(name: &str, default: SingleActionGesture) -> SingleActionGesture {
    match name {
        "None" => SingleActionGesture::None,
        "Seek" => SingleActionGesture::Seek,
        "PlayPause" => SingleActionGesture::PlayPause,
        "Switch" => SingleActionGesture::Switch,
        "Custom" => SingleActionGesture::Custom,
        _ => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_thirteen_keys() {
        assert_eq!(KEYS.len(), 13);
        assert!(KEYS.iter().all(|k| k.starts_with("pref_")));
    }

    #[test]
    fn every_default_matches_the_data_doc() {
        let d = GesturePrefs::default();
        assert!(d.gesture_volume_brightness);
        assert!(!d.swap_volume_and_brightness);
        assert!(d.gesture_horizontal_seek);
        assert!(!d.show_seekbar);
        assert_eq!(d.default_intro_length, 85);
        assert_eq!(d.skip_length, 10);
        assert!(!d.smooth_seek);
        assert_eq!(d.left_double_tap, SingleActionGesture::Seek);
        assert_eq!(d.center_double_tap, SingleActionGesture::PlayPause);
        assert_eq!(d.right_double_tap, SingleActionGesture::Seek);
        assert_eq!(d.media_previous, SingleActionGesture::Switch);
        assert_eq!(d.media_playpause, SingleActionGesture::PlayPause);
        assert_eq!(d.media_next, SingleActionGesture::Switch);
    }

    #[test]
    fn stored_names_parse_and_unknown_names_use_the_default() {
        assert_eq!(
            action_from_name("Custom", SingleActionGesture::Seek),
            SingleActionGesture::Custom
        );
        assert_eq!(
            action_from_name("Bogus", SingleActionGesture::Seek),
            SingleActionGesture::Seek
        );
    }
}
