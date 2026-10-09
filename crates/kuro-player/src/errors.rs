//! Player messages and the initial-episode error path (area 45 `PlayerActivity.setInitialEpisodeError`,
//! behaviour 13.1; T-45a-052 and the message keys of T-45a-037). Text is English. Localisation is
//! the UI layer's job, keyed by `PlayerMessage::key`.

/// A fixed player message with its localisation key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerMessage {
    /// Toast when autoplay is off and there is no next episode (`no_next_episode`).
    NoNextEpisode,
    /// Error when the hoster list of a switched episode is empty (`no_hosters`).
    NoHosters,
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
}

impl PlayerMessage {
    pub const fn key(self) -> &'static str {
        match self {
            Self::NoNextEpisode => "no_next_episode",
            Self::NoHosters => "no_hosters",
            Self::PictureSaved => "picture_saved",
            Self::HttpServerStartFailure => "http_server_start_failure",
            Self::ScreenshotFailed => "player_screenshot_failed",
            Self::AutoPlayOn => "enable_auto_play",
            Self::AutoPlayOff => "disable_auto_play",
            Self::ValueTooBig => "numeric_chooser_value_too_big",
            Self::ValueTooSmall => "numeric_chooser_value_too_small",
        }
    }

    pub const fn text(self) -> &'static str {
        match self {
            Self::NoNextEpisode => "Next Episode not found!",
            Self::NoHosters => "Hoster list is empty",
            Self::PictureSaved => "Picture saved",
            Self::HttpServerStartFailure => "Failed to start http server",
            Self::ScreenshotFailed => "Screenshot failed",
            Self::AutoPlayOn => "Auto-play is on",
            Self::AutoPlayOff => "Auto-play is off",
            Self::ValueTooBig => "Value too big",
            Self::ValueTooSmall => "Value too small",
        }
    }
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
        InitError::Localised(message) => message.text().to_owned(),
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
    fn message_keys_and_texts() {
        assert_eq!(PlayerMessage::NoNextEpisode.key(), "no_next_episode");
        assert_eq!(
            PlayerMessage::NoNextEpisode.text(),
            "Next Episode not found!"
        );
        assert_eq!(PlayerMessage::NoHosters.key(), "no_hosters");
        assert_eq!(PlayerMessage::NoHosters.text(), "Hoster list is empty");
        assert_eq!(PlayerMessage::PictureSaved.key(), "picture_saved");
        assert_eq!(PlayerMessage::PictureSaved.text(), "Picture saved");
        assert_eq!(
            PlayerMessage::HttpServerStartFailure.key(),
            "http_server_start_failure"
        );
        assert_eq!(
            PlayerMessage::HttpServerStartFailure.text(),
            "Failed to start http server"
        );
        assert_eq!(
            PlayerMessage::ScreenshotFailed.key(),
            "player_screenshot_failed"
        );
        assert_eq!(PlayerMessage::ScreenshotFailed.text(), "Screenshot failed");
        assert_eq!(PlayerMessage::AutoPlayOn.text(), "Auto-play is on");
        assert_eq!(PlayerMessage::AutoPlayOff.key(), "disable_auto_play");
        assert_eq!(PlayerMessage::ValueTooBig.text(), "Value too big");
        assert_eq!(
            PlayerMessage::ValueTooSmall.key(),
            "numeric_chooser_value_too_small"
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
