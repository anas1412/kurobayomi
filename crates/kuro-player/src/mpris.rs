//! MPRIS media controls, the replacement for `MediaSession` (area 45 `PlayerActivity.setupMediaSession`,
//! T-45a-043). This module is the pure part: which action each MPRIS button runs, the playback
//! status and the metadata. The D-Bus service (`org.mpris.MediaPlayer2.Player`) belongs to the
//! platform crate, which owns the D-Bus client. It is built there on these values.

use crate::enums::{CustomKeyCode, SingleActionGesture};

/// The four MPRIS `Player` methods the app answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MprisButton {
    Play,
    Pause,
    Next,
    Previous,
}

/// What a button does, as the window understands it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MprisAction {
    None,
    /// Relative seek in seconds: negative for Previous (left seek), positive for Next (right seek).
    Seek(f64),
    /// `Play` with the play/pause gesture.
    Unpause,
    /// `Pause` with the play/pause gesture.
    Pause,
    /// Toggle pause (Previous or Next with the play/pause gesture).
    PauseUnpause,
    /// Episode switch: `previous` for the previous episode, otherwise the next.
    SwitchEpisode {
        previous: bool,
    },
    /// Sends a custom key code to mpv with `keypress`.
    Keypress(CustomKeyCode),
}

/// The action for one button under its configured gesture. `skip_seconds` is the skip length
/// preference (`pref_skip_length_preference`, default 10).
pub fn action(button: MprisButton, gesture: SingleActionGesture, skip_seconds: f64) -> MprisAction {
    match (button, gesture) {
        (_, SingleActionGesture::None) => MprisAction::None,
        (MprisButton::Play, SingleActionGesture::PlayPause) => MprisAction::Unpause,
        (MprisButton::Pause, SingleActionGesture::PlayPause) => MprisAction::Pause,
        (MprisButton::Play | MprisButton::Pause, SingleActionGesture::Custom) => {
            MprisAction::Keypress(CustomKeyCode::MediaPlay)
        }
        (MprisButton::Play | MprisButton::Pause, _) => MprisAction::None,
        (MprisButton::Previous, SingleActionGesture::Seek) => MprisAction::Seek(-skip_seconds),
        (MprisButton::Next, SingleActionGesture::Seek) => MprisAction::Seek(skip_seconds),
        (MprisButton::Previous | MprisButton::Next, SingleActionGesture::PlayPause) => {
            MprisAction::PauseUnpause
        }
        (MprisButton::Previous, SingleActionGesture::Custom) => {
            MprisAction::Keypress(CustomKeyCode::MediaPrevious)
        }
        (MprisButton::Next, SingleActionGesture::Custom) => {
            MprisAction::Keypress(CustomKeyCode::MediaNext)
        }
        (MprisButton::Previous, SingleActionGesture::Switch) => {
            MprisAction::SwitchEpisode { previous: true }
        }
        (MprisButton::Next, SingleActionGesture::Switch) => {
            MprisAction::SwitchEpisode { previous: false }
        }
    }
}

/// A mini-player control, sent through the same channel as the MPRIS buttons. The codes are the
/// ones the Kotlin `pip_control` action uses: 1 pause, 2 play, 3 previous, 4 next, 5 skip (T-45a-048).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipControl {
    Pause,
    Play,
    Previous,
    Next,
    Skip,
}

impl PipControl {
    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            1 => Some(Self::Pause),
            2 => Some(Self::Play),
            3 => Some(Self::Previous),
            4 => Some(Self::Next),
            5 => Some(Self::Skip),
            _ => None,
        }
    }

    pub fn code(self) -> i64 {
        match self {
            Self::Pause => 1,
            Self::Play => 2,
            Self::Previous => 3,
            Self::Next => 4,
            Self::Skip => 5,
        }
    }
}

/// The seek of the mini-player skip button, always 10 s (`viewModel.seekBy(10)`), whatever the
/// skip length preference says.
pub const PIP_SKIP_SECONDS: f64 = 10.0;

/// What a mini-player control does. It is the action the MPRIS buttons produce, so one consumer
/// handles both (T-45a-048).
pub fn pip_action(control: PipControl) -> MprisAction {
    match control {
        PipControl::Pause => MprisAction::Pause,
        PipControl::Play => MprisAction::Unpause,
        PipControl::Previous => MprisAction::SwitchEpisode { previous: true },
        PipControl::Next => MprisAction::SwitchEpisode { previous: false },
        PipControl::Skip => MprisAction::Seek(PIP_SKIP_SECONDS),
    }
}

/// The channel the MPRIS buttons and the mini-player send their actions on. Unbounded, so a
/// producer never waits for the window.
pub fn action_channel() -> (
    async_channel::Sender<MprisAction>,
    async_channel::Receiver<MprisAction>,
) {
    async_channel::unbounded()
}

/// A `pip_control` action with `code` 1 to 5: sends its `MprisAction` on `tx`. An unknown code
/// sends nothing and returns `false`.
pub fn send_pip_control(tx: &async_channel::Sender<MprisAction>, code: i64) -> bool {
    PipControl::from_code(code)
        .is_some_and(|control| tx.try_send(pip_action(control)).is_ok())
}

/// The four mini-player buttons in order: previous (or skip -10 s), play/pause, next and close
/// (close is not a control code, the window closes itself).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MiniPlayerButtons {
    pub first: PipControl,
    pub first_enabled: bool,
    pub play_pause: PipControl,
    pub next_enabled: bool,
}

/// The mini-player row (T-45a-048). `first` is previous when `replace_with_previous` is set, else
/// skip. Previous is disabled at index 0 and Next at the last episode. The play/pause button sends
/// the opposite of the current state, so its icon swaps with `paused`.
pub fn mini_player_buttons(
    replace_with_previous: bool,
    index: usize,
    count: usize,
    paused: bool,
) -> MiniPlayerButtons {
    MiniPlayerButtons {
        first: if replace_with_previous {
            PipControl::Previous
        } else {
            PipControl::Skip
        },
        first_enabled: !replace_with_previous || index > 0,
        play_pause: if paused {
            PipControl::Play
        } else {
            PipControl::Pause
        },
        next_enabled: index + 1 < count,
    }
}

/// `PlaybackStatus` for the paused state.
pub fn playback_status(paused: bool) -> &'static str {
    if paused { "Paused" } else { "Playing" }
}

/// The metadata shown by the desktop: the episode title as the track title, the anime title as the
/// album. Keys are the MPRIS `xesam` names.
pub fn metadata(anime_title: &str, episode_title: &str) -> Vec<(&'static str, String)> {
    vec![
        ("xesam:title", episode_title.to_owned()),
        ("xesam:album", anime_title.to_owned()),
    ]
}

/// The `keypress` argument for a custom key code, as Kotlin sends it: the code in hex, such as
/// `0x10005` (`CustomKeyCodes.*.keyCode`).
pub fn keypress_args(code: CustomKeyCode) -> [String; 2] {
    ["keypress".to_owned(), format!("{:#x}", code as u32)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pip_codes_reach_the_same_channel_as_the_mpris_buttons() {
        let (tx, rx) = action_channel();
        for code in 1..=5 {
            assert!(send_pip_control(&tx, code));
        }
        assert!(!send_pip_control(&tx, 0));
        assert!(!send_pip_control(&tx, 6));
        // An MPRIS button goes down the same channel.
        tx.try_send(action(MprisButton::Pause, SingleActionGesture::PlayPause, 10.0))
            .unwrap();
        let got: Vec<MprisAction> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert_eq!(
            got,
            vec![
                MprisAction::Pause,
                MprisAction::Unpause,
                MprisAction::SwitchEpisode { previous: true },
                MprisAction::SwitchEpisode { previous: false },
                MprisAction::Seek(10.0),
                MprisAction::Pause,
            ]
        );
    }

    #[test]
    fn the_mini_player_row_sends_the_codes_its_buttons_show() {
        let row = mini_player_buttons(true, 1, 3, false);
        assert_eq!(pip_action(row.first), MprisAction::SwitchEpisode { previous: true });
        assert_eq!(pip_action(row.play_pause), MprisAction::Pause);
        let row = mini_player_buttons(false, 0, 3, true);
        assert_eq!(pip_action(row.first), MprisAction::Seek(10.0));
        assert_eq!(pip_action(row.play_pause), MprisAction::Unpause);
    }

    #[test]
    fn seek_buttons_use_the_skip_length_in_both_directions() {
        assert_eq!(
            action(MprisButton::Previous, SingleActionGesture::Seek, 10.0),
            MprisAction::Seek(-10.0)
        );
        assert_eq!(
            action(MprisButton::Next, SingleActionGesture::Seek, 30.0),
            MprisAction::Seek(30.0)
        );
    }

    #[test]
    fn play_and_pause_follow_the_play_pause_gesture() {
        assert_eq!(
            action(MprisButton::Play, SingleActionGesture::PlayPause, 10.0),
            MprisAction::Unpause
        );
        assert_eq!(
            action(MprisButton::Pause, SingleActionGesture::PlayPause, 10.0),
            MprisAction::Pause
        );
        assert_eq!(
            action(MprisButton::Pause, SingleActionGesture::Seek, 10.0),
            MprisAction::None
        );
    }

    #[test]
    fn switch_gesture_changes_episode_by_button() {
        assert_eq!(
            action(MprisButton::Previous, SingleActionGesture::Switch, 10.0),
            MprisAction::SwitchEpisode { previous: true }
        );
        assert_eq!(
            action(MprisButton::Next, SingleActionGesture::Switch, 10.0),
            MprisAction::SwitchEpisode { previous: false }
        );
    }

    #[test]
    fn custom_gesture_sends_the_three_key_codes() {
        assert_eq!(
            action(MprisButton::Previous, SingleActionGesture::Custom, 10.0),
            MprisAction::Keypress(CustomKeyCode::MediaPrevious)
        );
        assert_eq!(
            action(MprisButton::Play, SingleActionGesture::Custom, 10.0),
            MprisAction::Keypress(CustomKeyCode::MediaPlay)
        );
        assert_eq!(
            action(MprisButton::Next, SingleActionGesture::Custom, 10.0),
            MprisAction::Keypress(CustomKeyCode::MediaNext)
        );
        assert_eq!(
            keypress_args(CustomKeyCode::MediaPlay),
            ["keypress", "0x10005"]
        );
    }

    #[test]
    fn none_gesture_does_nothing_for_any_button() {
        for button in [
            MprisButton::Play,
            MprisButton::Pause,
            MprisButton::Next,
            MprisButton::Previous,
        ] {
            assert_eq!(
                action(button, SingleActionGesture::None, 10.0),
                MprisAction::None
            );
        }
    }

    #[test]
    fn status_and_metadata() {
        assert_eq!(playback_status(true), "Paused");
        assert_eq!(playback_status(false), "Playing");
        assert_eq!(
            metadata("Show", "Pilot"),
            vec![
                ("xesam:title", "Pilot".to_owned()),
                ("xesam:album", "Show".to_owned()),
            ]
        );
    }

    #[test]
    fn pip_codes_round_trip_and_unknown_codes_are_dropped() {
        for code in 1..=5 {
            assert_eq!(
                PipControl::from_code(code).map(PipControl::code),
                Some(code)
            );
        }
        assert_eq!(PipControl::from_code(0), None);
        assert_eq!(PipControl::from_code(6), None);
    }

    #[test]
    fn mini_player_previous_and_next_ends_are_disabled() {
        let first = mini_player_buttons(true, 0, 3, false);
        assert_eq!(first.first, PipControl::Previous);
        assert!(!first.first_enabled);
        assert!(first.next_enabled);
        assert_eq!(first.play_pause, PipControl::Pause);

        let last = mini_player_buttons(true, 2, 3, true);
        assert!(last.first_enabled);
        assert!(!last.next_enabled);
        assert_eq!(last.play_pause, PipControl::Play);
    }

    #[test]
    fn mini_player_without_replace_skips_and_never_disables_it() {
        let row = mini_player_buttons(false, 0, 1, false);
        assert_eq!(row.first, PipControl::Skip);
        assert!(row.first_enabled);
        assert!(!row.next_enabled);
    }
}
