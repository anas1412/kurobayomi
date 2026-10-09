//! Gestures page rows for seeking (task T-48a-022) and double tap (task T-48a-023).
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::Preference;
use crate::settings_ui::player::controls::info_row;
use crate::settings_ui::player::gestures::{option_labels, Slot};
use crate::settings_ui::rows::{enum_choice, group, int_choice, slider, switch, switch_sub};

/// Intro length range in seconds. 0 disables the intro button.
pub const INTRO_LENGTH_RANGE: std::ops::RangeInclusive<i32> = 0..=255;

/// Double-tap skip length options. 0 is "Disable".
pub const SKIP_LENGTHS: [(i32, &str); 6] = [(30, "30s"), (20, "20s"), (10, "10s"), (5, "5s"), (3, "3s"), (0, "Disable")];

pub const SEEKING_SUBTITLE: &str = "Swipe left or right to rewind or fast-forward";
pub const PRECISE_SUBTITLE: &str = "When enabled, seeking will not focus on keyframes, leading to slower but precise seeking";

/// The Seeking and Double tap groups (the sliders and media controls groups are tasks 021 and 024).
pub fn rows(prefs: &Prefs) -> Vec<Preference> {
    let g = &prefs.gestures;
    let intro = format!("{}s", g.default_intro_length.get());
    vec![
        group(
            "Seeking",
            vec![
                switch_sub("Enable Horizontal Seek Gesture", SEEKING_SUBTITLE, &g.gesture_horizontal_seek, true),
                switch("Show seekbar when seeking", &g.show_seek_bar, true),
                slider("Default skip intro length", &g.default_intro_length, INTRO_LENGTH_RANGE, Some(intro.as_str()), true),
                int_choice("Double tap to skip length", &g.skip_length_preference, &SKIP_LENGTHS, true),
                switch_sub("Enable precise seeking", PRECISE_SUBTITLE, &g.player_smooth_seek, true),
            ],
        ),
        group(
            "Double tap",
            vec![
                enum_choice("Double tap (left)", &g.left_double_tap_gesture, &option_labels(Slot::DoubleTapLeft), true),
                enum_choice("Double tap (center)", &g.center_double_tap_gesture, &option_labels(Slot::DoubleTapCenter), true),
                enum_choice("Double tap (right)", &g.right_double_tap_gesture, &option_labels(Slot::DoubleTapRight), true),
                info_row(),
            ],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pref_model::model::PreferenceItem;
    use kuro_prefs::player::SingleActionGesture;

    #[test]
    fn skip_length_defaults_to_10_seconds_with_disable_last() {
        let prefs = Prefs::in_memory();
        let tree = rows(&prefs);
        let Preference::Group(seeking) = &tree[0] else { panic!("group") };
        let PreferenceItem::BasicList { value, entries, .. } = &seeking.items[3] else { panic!("list") };
        assert_eq!(value, "10");
        assert_eq!(entries.last().map(|(_, l)| l.as_str()), Some("Disable"));
    }

    #[test]
    fn intro_length_slider_takes_0_to_255_and_shows_seconds() {
        let prefs = Prefs::in_memory();
        let tree = rows(&prefs);
        let Preference::Group(seeking) = &tree[0] else { panic!("group") };
        let PreferenceItem::Slider { value, range, common, .. } = &seeking.items[2] else { panic!("slider") };
        assert_eq!(*value, 85);
        assert_eq!(*range.start(), 0);
        assert_eq!(*range.end(), 255);
        assert_eq!(common.subtitle.as_deref(), Some("85s"));
    }

    #[test]
    fn double_tap_defaults_are_seek_playpause_seek() {
        let prefs = Prefs::in_memory();
        assert_eq!(prefs.gestures.left_double_tap_gesture.get(), SingleActionGesture::Seek);
        assert_eq!(prefs.gestures.center_double_tap_gesture.get(), SingleActionGesture::PlayPause);
        assert_eq!(prefs.gestures.right_double_tap_gesture.get(), SingleActionGesture::Seek);
        let tree = rows(&prefs);
        let Preference::Group(double) = &tree[1] else { panic!("group") };
        assert_eq!(double.items[0].title(), "Double tap (left)");
        assert_eq!(double.items.len(), 4);
    }

    #[test]
    fn the_centre_double_tap_offers_three_actions() {
        let prefs = Prefs::in_memory();
        let tree = rows(&prefs);
        let Preference::Group(double) = &tree[1] else { panic!("group") };
        let PreferenceItem::BasicList { entries, .. } = &double.items[1] else { panic!("list") };
        assert_eq!(entries.len(), 3);
    }
}
