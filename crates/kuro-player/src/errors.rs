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
}

impl PlayerMessage {
    pub const fn key(self) -> &'static str {
        match self {
            Self::NoNextEpisode => "no_next_episode",
            Self::NoHosters => "no_hosters",
        }
    }

    pub const fn text(self) -> &'static str {
        match self {
            Self::NoNextEpisode => "Next Episode not found!",
            Self::NoHosters => "Hoster list is empty",
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
