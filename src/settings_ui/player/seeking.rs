//! Gestures page rows for seeking (task T-48a-022) and double tap (task T-48a-023).
use std::rc::Rc;

use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::player::controls::info_row;
use crate::settings_ui::player::gestures::{option_labels, Slot};
use crate::settings_ui::player::intro;
use crate::settings_ui::rows::{enum_choice, group, info, int_choice, switch, switch_sub, text};

/// Intro length range in seconds. 0 disables the intro button.
pub const INTRO_LENGTH_RANGE: std::ops::RangeInclusive<i32> = 0..=255;

/// Double-tap skip length options. 0 is "Disable".
pub const SKIP_LENGTHS: [(i32, &str); 6] = [(30, "30s"), (20, "20s"), (10, "10s"), (5, "5s"), (3, "3s"), (0, "Disable")];

pub const SEEKING_SUBTITLE: &str = "Swipe left or right to rewind or fast-forward";
pub const PRECISE_SUBTITLE: &str = "When enabled, seeking will not focus on keyframes, leading to slower but precise seeking";

/// "Default skip intro length": an action row whose subtitle is the stored length; a tap opens the spin dialog.
pub fn intro_row(prefs: &Prefs) -> PreferenceItem {
    let pref = prefs.gestures.default_intro_length.clone();
    let mut row = text("Default skip intro length", Some(&intro::spin_text(pref.get())), true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(Rc::new(move || intro::show(pref.clone())));
    }
    row
}

/// The Media controls group: what the previous, play/pause and next media keys do.
pub fn media_group(prefs: &Prefs) -> Preference {
    let g = &prefs.gestures;
    group(
        "Media controls",
        vec![
            enum_choice("Previous", &g.media_previous_gesture, &option_labels(Slot::MediaPrevious), true),
            enum_choice("Play/Pause", &g.media_play_pause_gesture, &option_labels(Slot::MediaPlayPause), true),
            enum_choice("Next", &g.media_next_gesture, &option_labels(Slot::MediaNext), true),
            info(MEDIA_INFO),
        ],
    )
}

pub const MEDIA_INFO: &str = "When a media control is set to \"Custom\", it can be bound through input.conf. The key names are kurobayomi-media-previous, kurobayomi-media-play-pause and kurobayomi-media-next.";

/// The whole gestures page: Seeking, Double tap and Media controls.
pub fn page_rows(prefs: &Prefs) -> Vec<Preference> {
    let mut tree = rows(prefs);
    tree.push(media_group(prefs));
    tree
}

/// The Seeking and Double tap groups.
pub fn rows(prefs: &Prefs) -> Vec<Preference> {
    let g = &prefs.gestures;
    vec![
        group(
            "Seeking",
            vec![
                switch_sub("Enable Horizontal Seek Gesture", SEEKING_SUBTITLE, &g.gesture_horizontal_seek, true),
                switch("Show seekbar when seeking", &g.show_seek_bar, true),
                intro_row(prefs),
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
    fn intro_length_is_an_action_row_with_the_stored_seconds_and_writes_nothing_until_a_dialog_says_so() {
        let prefs = Prefs::in_memory();
        let tree = rows(&prefs);
        let Preference::Group(seeking) = &tree[0] else { panic!("group") };
        let PreferenceItem::Text { common, on_click } = &seeking.items[2] else { panic!("an action row, not a slider") };
        assert_eq!(common.title, "Default skip intro length");
        assert_eq!(common.subtitle.as_deref(), Some("85s"));
        assert!(on_click.is_some(), "a tap opens the dialog");
        assert_eq!((*INTRO_LENGTH_RANGE.start(), *INTRO_LENGTH_RANGE.end()), (0, 255));
        prefs.gestures.default_intro_length.set(120).unwrap();
        let PreferenceItem::Text { common, .. } = intro_row(&prefs) else { panic!("text row") };
        assert_eq!(common.subtitle.as_deref(), Some("120s"), "the subtitle follows the stored value");
    }

    #[test]
    fn the_page_has_the_three_spec_groups_with_their_rows() {
        let tree = page_rows(&Prefs::in_memory());
        let shape: Vec<(String, Vec<String>)> = tree
            .iter()
            .map(|e| match e {
                Preference::Group(g) => (g.title.clone(), g.items.iter().map(|i| i.title().to_owned()).collect()),
                Preference::Item(_) => panic!("only groups"),
            })
            .collect();
        let titles: Vec<&str> = shape.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(titles, ["Seeking", "Double tap", "Media controls"]);
        assert_eq!(shape[2].1[..3], ["Previous", "Play/Pause", "Next"]);
        crate::ui::with_spec(|spec| {
            let screen = spec.screen("settings_player_gestures").expect("screen");
            for (group, (_, rows)) in screen.groups.iter().zip(&shape) {
                let spec_titles: Vec<String> = group.items.iter().map(crate::pref_model::spec_convert::item_title).collect();
                assert_eq!(spec_titles.len(), rows.len(), "{:?}", group.title);
                for (spec_title, row) in spec_titles.iter().zip(rows).filter(|(t, _)| t.len() < 60) {
                    assert_eq!(spec_title, row);
                }
            }
        });
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
