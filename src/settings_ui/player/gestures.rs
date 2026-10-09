//! Single-action gestures: which action each slot offers, what each action does, and the mpv key names that the
//! `Custom` action sends (tasks T-48a-025 and the option lists of T-48a-023).
use kuro_prefs::player::SingleActionGesture;

/// A place a gesture can happen: the double-tap sides and centre, and the three media buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    DoubleTapLeft,
    DoubleTapCenter,
    DoubleTapRight,
    MediaPrevious,
    MediaPlayPause,
    MediaNext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Previous,
    Next,
}

/// What the player does for one action. `Seek` carries signed seconds (negative = back).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Nothing,
    Seek(i64),
    PlayPause,
    ChangeEpisode(Step),
    /// The mpv key name the user binds in `input.conf`.
    SendKey(&'static str),
}

/// The mpv key name a slot sends for `Custom`.
pub const fn key_name(slot: Slot) -> &'static str {
    match slot {
        Slot::DoubleTapLeft => "kurobayomi-double-tap-left",
        Slot::DoubleTapCenter => "kurobayomi-double-tap-center",
        Slot::DoubleTapRight => "kurobayomi-double-tap-right",
        Slot::MediaPrevious => "kurobayomi-media-previous",
        Slot::MediaPlayPause => "kurobayomi-media-play-pause",
        Slot::MediaNext => "kurobayomi-media-next",
    }
}

const EDGE: [SingleActionGesture; 5] = [
    SingleActionGesture::None,
    SingleActionGesture::Seek,
    SingleActionGesture::PlayPause,
    SingleActionGesture::Switch,
    SingleActionGesture::Custom,
];
const CENTRE: [SingleActionGesture; 3] = [SingleActionGesture::None, SingleActionGesture::PlayPause, SingleActionGesture::Custom];

/// The actions a slot offers, in list order.
pub fn options(slot: Slot) -> &'static [SingleActionGesture] {
    match slot {
        Slot::DoubleTapCenter | Slot::MediaPlayPause => &CENTRE,
        _ => &EDGE,
    }
}

/// Labels of the actions (Android `SingleActionGesture.stringRes`).
pub const fn label(action: SingleActionGesture) -> &'static str {
    match action {
        SingleActionGesture::None => "None",
        SingleActionGesture::Seek => "Seek",
        SingleActionGesture::PlayPause => "Play/Pause",
        SingleActionGesture::Switch => "Change episode",
        SingleActionGesture::Custom => "Custom",
    }
}

/// `(action, label)` pairs for a list row of `slot`.
pub fn option_labels(slot: Slot) -> Vec<(SingleActionGesture, &'static str)> {
    options(slot).iter().map(|action| (*action, label(*action))).collect()
}

pub fn allowed(slot: Slot, action: SingleActionGesture) -> bool {
    options(slot).contains(&action)
}

/// Back for the left and previous-media slots, forward for the right and next-media slots.
const fn side(slot: Slot) -> Option<Step> {
    match slot {
        Slot::DoubleTapLeft | Slot::MediaPrevious => Some(Step::Previous),
        Slot::DoubleTapRight | Slot::MediaNext => Some(Step::Next),
        Slot::DoubleTapCenter | Slot::MediaPlayPause => None,
    }
}

/// The one command an action produces in a slot. A pair the slot does not offer, a seek of zero seconds, or a seek
/// or episode change in the centre gives [`Command::Nothing`].
pub fn dispatch(slot: Slot, action: SingleActionGesture, skip_secs: i32) -> Command {
    if !allowed(slot, action) {
        return Command::Nothing;
    }
    match action {
        SingleActionGesture::None => Command::Nothing,
        SingleActionGesture::Seek => match (side(slot), skip_secs) {
            (_, secs) if secs <= 0 => Command::Nothing,
            (Some(Step::Previous), secs) => Command::Seek(-i64::from(secs)),
            (Some(Step::Next), secs) => Command::Seek(i64::from(secs)),
            (None, _) => Command::Nothing,
        },
        SingleActionGesture::PlayPause => Command::PlayPause,
        SingleActionGesture::Switch => side(slot).map_or(Command::Nothing, Command::ChangeEpisode),
        SingleActionGesture::Custom => Command::SendKey(key_name(slot)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_SLOTS: [Slot; 6] = [
        Slot::DoubleTapLeft,
        Slot::DoubleTapCenter,
        Slot::DoubleTapRight,
        Slot::MediaPrevious,
        Slot::MediaPlayPause,
        Slot::MediaNext,
    ];

    #[test]
    fn key_names_are_the_six_desktop_names() {
        assert_eq!(key_name(Slot::DoubleTapLeft), "kurobayomi-double-tap-left");
        assert_eq!(key_name(Slot::DoubleTapCenter), "kurobayomi-double-tap-center");
        assert_eq!(key_name(Slot::DoubleTapRight), "kurobayomi-double-tap-right");
        assert_eq!(key_name(Slot::MediaPrevious), "kurobayomi-media-previous");
        assert_eq!(key_name(Slot::MediaPlayPause), "kurobayomi-media-play-pause");
        assert_eq!(key_name(Slot::MediaNext), "kurobayomi-media-next");
    }

    #[test]
    fn seek_goes_back_on_the_left_and_forward_on_the_right() {
        assert_eq!(dispatch(Slot::DoubleTapLeft, SingleActionGesture::Seek, 10), Command::Seek(-10));
        assert_eq!(dispatch(Slot::DoubleTapRight, SingleActionGesture::Seek, 10), Command::Seek(10));
        assert_eq!(dispatch(Slot::MediaPrevious, SingleActionGesture::Seek, 5), Command::Seek(-5));
        assert_eq!(dispatch(Slot::MediaNext, SingleActionGesture::Seek, 5), Command::Seek(5));
    }

    #[test]
    fn a_zero_skip_length_disables_seeking() {
        assert_eq!(dispatch(Slot::DoubleTapRight, SingleActionGesture::Seek, 0), Command::Nothing);
    }

    #[test]
    fn switch_changes_episode_back_on_the_left_and_forward_on_the_right() {
        assert_eq!(dispatch(Slot::DoubleTapLeft, SingleActionGesture::Switch, 10), Command::ChangeEpisode(Step::Previous));
        assert_eq!(dispatch(Slot::MediaNext, SingleActionGesture::Switch, 10), Command::ChangeEpisode(Step::Next));
    }

    #[test]
    fn custom_sends_the_slot_key_name_and_none_does_nothing() {
        assert_eq!(dispatch(Slot::DoubleTapCenter, SingleActionGesture::Custom, 10), Command::SendKey("kurobayomi-double-tap-center"));
        assert_eq!(dispatch(Slot::DoubleTapRight, SingleActionGesture::None, 10), Command::Nothing);
    }

    #[test]
    fn every_offered_pair_dispatches_one_command_and_a_disallowed_pair_is_nothing() {
        for slot in ALL_SLOTS {
            for action in options(slot) {
                assert!(allowed(slot, *action));
                let _ = dispatch(slot, *action, 10);
            }
        }
        assert!(!allowed(Slot::DoubleTapCenter, SingleActionGesture::Seek));
        assert_eq!(dispatch(Slot::DoubleTapCenter, SingleActionGesture::Seek, 10), Command::Nothing);
        assert_eq!(dispatch(Slot::DoubleTapCenter, SingleActionGesture::Switch, 10), Command::Nothing);
    }

    #[test]
    fn option_lists_match_the_spec() {
        assert_eq!(option_labels(Slot::DoubleTapLeft).iter().map(|(_, l)| *l).collect::<Vec<_>>(), ["None", "Seek", "Play/Pause", "Change episode", "Custom"]);
        assert_eq!(option_labels(Slot::DoubleTapCenter).iter().map(|(_, l)| *l).collect::<Vec<_>>(), ["None", "Play/Pause", "Custom"]);
        assert_eq!(option_labels(Slot::MediaPlayPause).len(), 3);
    }
}
