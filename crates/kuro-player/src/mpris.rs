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
}
