//! Player messages and the initial-episode error path (area 45 `PlayerActivity.setInitialEpisodeError`,
//! behaviour 13.1; T-45a-052, the message keys of T-45a-037, T-45b-061). Text comes from the
//! `kuro-i18n` catalogue, keyed by `PlayerMessage::key`.

use kuro_i18n::{Arg, MsgKey, tr, trf};

/// A fixed player message with its localisation key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerMessage {
    /// Toast when autoplay is off and there is no next episode (`no_next_episode`).
    NoNextEpisode,
    /// Toast when there is no previous episode (`no_prev_episode`).
    NoPrevEpisode,
    /// Error when the hoster list of a switched episode is empty (`no_hosters`).
    NoHosters,
    /// `no_episode_loaded`: the first load or a later episode load failed.
    NoEpisodeLoaded,
    /// `no_available_videos`: every hoster and video was tried.
    NoAvailableVideos,
    /// `select_hoster_from_empty_list`: the opening hoster list parsed empty.
    SelectHosterFromEmptyList,
    /// Toast after a screenshot is saved (`picture_saved`).
    PictureSaved,
    /// Toast when the local stream server cannot start (`http_server_start_failure`).
    HttpServerStartFailure,
    /// Shown in the screenshot sheet when mpv wrote no file (`player_screenshot_failed`, new key).
    ScreenshotFailed,
    /// Pill text when auto-play is switched on (`enable_auto_play`).
    AutoPlayOn,
    /// Pill text when auto-play is switched off (`disable_auto_play`).
    AutoPlayOff,
    /// Numeric chooser error above the maximum (`numeric_chooser_value_too_big`).
    ValueTooBig,
    /// Numeric chooser error below the minimum (`numeric_chooser_value_too_small`).
    ValueTooSmall,
    /// Toast when the sleep timer fires (`toast_sleep_timer_ended`).
    SleepTimerEnded,
    /// Skip button label during the Netflix-style countdown (`player_aniskip_dontskip`).
    AniskipDontSkip,
}

impl PlayerMessage {
    pub const fn key(self) -> &'static str {
        match self {
            Self::NoNextEpisode => "no_next_episode",
            Self::NoPrevEpisode => "no_prev_episode",
            Self::NoHosters => "no_hosters",
            Self::NoEpisodeLoaded => "no_episode_loaded",
            Self::NoAvailableVideos => "no_available_videos",
            Self::SelectHosterFromEmptyList => "select_hoster_from_empty_list",
            Self::PictureSaved => "picture_saved",
            Self::HttpServerStartFailure => "http_server_start_failure",
            Self::ScreenshotFailed => "player_screenshot_failed",
            Self::AutoPlayOn => "enable_auto_play",
            Self::AutoPlayOff => "disable_auto_play",
            Self::ValueTooBig => "numeric_chooser_value_too_big",
            Self::ValueTooSmall => "numeric_chooser_value_too_small",
            Self::SleepTimerEnded => "toast_sleep_timer_ended",
            Self::AniskipDontSkip => "player_aniskip_dontskip",
        }
    }

    /// The text in the active language. `ScreenshotFailed` has no catalogue key yet, so it keeps
    /// its English text until the key is added to the FTL files.
    pub fn text(self) -> String {
        let key = match self {
            Self::NoNextEpisode => MsgKey::no_next_episode,
            Self::NoPrevEpisode => MsgKey::no_prev_episode,
            Self::NoHosters => MsgKey::no_hosters,
            Self::NoEpisodeLoaded => MsgKey::no_episode_loaded,
            Self::NoAvailableVideos => MsgKey::no_available_videos,
            Self::SelectHosterFromEmptyList => MsgKey::select_hoster_from_empty_list,
            Self::PictureSaved => MsgKey::picture_saved,
            Self::HttpServerStartFailure => MsgKey::http_server_start_failure,
            Self::ScreenshotFailed => return "Screenshot failed".to_owned(),
            Self::AutoPlayOn => MsgKey::enable_auto_play,
            Self::AutoPlayOff => MsgKey::disable_auto_play,
            Self::ValueTooBig => MsgKey::numeric_chooser_value_too_big,
            Self::ValueTooSmall => MsgKey::numeric_chooser_value_too_small,
            Self::SleepTimerEnded => MsgKey::toast_sleep_timer_ended,
            Self::AniskipDontSkip => MsgKey::player_aniskip_dontskip,
        };
        tr(key)
    }
}

/// Seek text after an auto-skip (`player_intro_skipped`); `chapter` is the chapter name.
pub fn intro_skipped_text(chapter: &str) -> String {
    trf(MsgKey::player_intro_skipped, &[("arg1", Arg::Str(chapter))])
}

/// Seek text after the countdown or a manual skip (`player_aniskip_skip`).
pub fn aniskip_skip_text(chapter: &str) -> String {
    trf(MsgKey::player_aniskip_skip, &[("arg1", Arg::Str(chapter))])
}

/// Toast during the Netflix-style countdown (`player_aniskip_dontskip_toast`).
///
/// DECISION: the Kotlin prefixes this with a hard-coded "Skip Intro: ". The FTL has no key for
/// that prefix and this crate does not own the FTL, so the prefix is left out until a localised
/// `player_aniskip_toast_prefix` key exists.
pub fn aniskip_countdown_toast(chapter: &str, seconds: i64) -> String {
    trf(
        MsgKey::player_aniskip_dontskip_toast,
        &[("arg1", Arg::Str(chapter)), ("arg2", Arg::Int(seconds))],
    )
}

/// Why the first episode could not be shown. A localised error (`ExceptionWithStringResource`)
/// shows its message. Any other error shows its raw message, if it has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InitError {
    Localised(PlayerMessage),
    Other(Option<String>),
}

/// The toast or error text for an init error. A missing raw message reads "Unknown error".
pub fn init_error_text(error: &InitError) -> String {
    match error {
        InitError::Localised(message) => message.text(),
        InitError::Other(Some(raw)) => raw.clone(),
        InitError::Other(None) => "Unknown error".to_owned(),
    }
}

/// Logs the init error at error level and returns the text to show. The caller closes the window
/// after this (`finish()`).
pub fn report_init_error(error: &InitError) -> String {
    let text = init_error_text(error);
    tracing::error!(error = %text, "initial episode failed to load");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_keys_and_texts_come_from_the_catalogue() {
        let cases = [
            (
                PlayerMessage::NoNextEpisode,
                "no_next_episode",
                "Next Episode not found!",
            ),
            (
                PlayerMessage::NoPrevEpisode,
                "no_prev_episode",
                "Previous Episode not found!",
            ),
            (
                PlayerMessage::NoHosters,
                "no_hosters",
                "Hoster list is empty",
            ),
            (
                PlayerMessage::NoEpisodeLoaded,
                "no_episode_loaded",
                "No episode loaded",
            ),
            (
                PlayerMessage::NoAvailableVideos,
                "no_available_videos",
                "No available videos",
            ),
            (
                PlayerMessage::SelectHosterFromEmptyList,
                "select_hoster_from_empty_list",
                "Hoster selected from empty list",
            ),
            (
                PlayerMessage::PictureSaved,
                "picture_saved",
                "Picture saved",
            ),
            (
                PlayerMessage::HttpServerStartFailure,
                "http_server_start_failure",
                "Failed to start http server",
            ),
            (
                PlayerMessage::ScreenshotFailed,
                "player_screenshot_failed",
                "Screenshot failed",
            ),
            (
                PlayerMessage::AutoPlayOn,
                "enable_auto_play",
                "Auto-play is on",
            ),
            (
                PlayerMessage::AutoPlayOff,
                "disable_auto_play",
                "Auto-play is off",
            ),
            (
                PlayerMessage::ValueTooBig,
                "numeric_chooser_value_too_big",
                "Value too big",
            ),
            (
                PlayerMessage::ValueTooSmall,
                "numeric_chooser_value_too_small",
                "Value too small",
            ),
            (
                PlayerMessage::SleepTimerEnded,
                "toast_sleep_timer_ended",
                "Sleep timer ended",
            ),
            (
                PlayerMessage::AniskipDontSkip,
                "player_aniskip_dontskip",
                "Don't skip",
            ),
        ];
        for (message, key, text) in cases {
            assert_eq!(message.key(), key);
            assert_eq!(message.text(), text);
        }
    }

    #[test]
    fn templated_messages_fill_their_arguments() {
        assert_eq!(intro_skipped_text("Opening"), "Opening skipped");
        assert_eq!(aniskip_skip_text("Ending"), "Ending skipped");
        assert_eq!(
            aniskip_countdown_toast("Opening", 5),
            "Skipping Opening in 5 seconds"
        );
    }

    #[test]
    fn localised_error_shows_its_message() {
        let e = InitError::Localised(PlayerMessage::NoHosters);
        assert_eq!(init_error_text(&e), "Hoster list is empty");
    }

    #[test]
    fn raw_error_shows_message_or_unknown() {
        assert_eq!(
            init_error_text(&InitError::Other(Some("boom".to_owned()))),
            "boom"
        );
        assert_eq!(init_error_text(&InitError::Other(None)), "Unknown error");
    }

    #[test]
    fn report_returns_the_same_text() {
        let e = InitError::Other(None);
        assert_eq!(report_init_error(&e), "Unknown error");
    }
}
