//! Error text for the details page: `ExceptionFormatter.formattedMessage` and the chapter-fetch message.
//! Upstream: `app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt:16-38`.
//! Errors are typed here, so the mapping does not depend on runtime class names.

use kuro_i18n::{MsgKey, tr};
use kuro_source::SourceError;

use crate::browse::errors::{GENERIC, OFFLINE, Trouble, plain_message, says};

/// A failure from the source fetch or the store. The variants are the cases `formattedMessage` distinguishes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DetailError {
    /// An HTTP status code.
    Http(i64),
    /// `UnknownHostException`. `host` is the exception message (the host name).
    UnknownHost { online: bool, host: String },
    /// `NoChaptersException`: the source returned no chapters for a non-local manga.
    NoChapters,
    /// The source extension is not installed.
    SourceNotInstalled,
    /// `NoEpisodesException`: a refresh of an `Episodes` entry found no episodes.
    NoEpisodes,
    /// `NoSeasonsException`: a refresh of a `Seasons` entry found no seasons.
    NoSeasons,
    /// `CancellationException`: the page was closed or the refresh was replaced. Never shown.
    Cancelled,
    /// A failure of the source, already worded for the user (`browse::errors::plain_message`). The technical text is in
    /// the log.
    Plain(String),
    /// Any other error: `class` is the simple class name, `message` the exception message.
    Other {
        class: String,
        message: Option<String>,
    },
}

impl DetailError {
    /// The page error of a failed call of the source `source` (its bare name). The error text goes to the log; the page
    /// shows a sentence. An uninstalled source, a cancellation and HTTP 103 keep their own kinds.
    pub fn from_source(source: &str, error: SourceError) -> Self {
        match error {
            SourceError::NotInstalled(_) => Self::SourceNotInstalled,
            SourceError::Cancelled => Self::Cancelled,
            SourceError::Http(103) => Self::Http(103),
            other => Self::Plain(plain_message(source, &other)),
        }
    }

    /// `formattedMessage`: the text of a banner or toast for this error.
    pub fn user_message(&self) -> String {
        match self {
            Self::Http(_) => says("This source", Trouble::Broken),
            Self::UnknownHost { online: false, .. } => OFFLINE.to_owned(),
            Self::UnknownHost { online: true, .. } => says("This source", Trouble::Broken),
            Self::Plain(text) => text.clone(),
            Self::NoChapters => tr(MsgKey::no_results_found),
            Self::SourceNotInstalled => tr(MsgKey::loader_not_implemented_error),
            Self::NoEpisodes => tr(MsgKey::no_episodes_error),
            Self::NoSeasons => tr(MsgKey::no_seasons_error),
            Self::Cancelled => String::new(),
            Self::Other { class, message } => {
                tracing::warn!(class = class.as_str(), message = message.as_deref().unwrap_or(""), "an entry page failure");
                GENERIC.to_owned()
            }
        }
    }

    /// The snackbar text of a failed refresh (`AnimeScreenModel.kt:350-363`). `None` for a cancellation, which is not shown.
    pub fn refresh_message(&self) -> Option<String> {
        match self {
            Self::Cancelled => None,
            other => Some(other.user_message()),
        }
    }

    /// HTTP 103 (Early Hints) is not an error. The details fetch ignores it silently (E-FETCH-1).
    pub fn is_ignored(&self) -> bool {
        matches!(self, Self::Http(103))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_http_error_is_a_plain_sentence_without_the_code() {
        let text = DetailError::Http(404).user_message();
        assert!(text.contains("isn't working right now") && !text.contains("404") && !text.contains("HTTP"), "{text}");
    }

    #[test]
    fn a_source_error_becomes_a_sentence_except_the_kinds_the_page_treats_on_their_own() {
        assert_eq!(DetailError::from_source("Comix", SourceError::NotInstalled(1)), DetailError::SourceNotInstalled);
        assert_eq!(DetailError::from_source("Comix", SourceError::Cancelled), DetailError::Cancelled);
        assert!(DetailError::from_source("Comix", SourceError::Http(103)).is_ignored());
        let text = DetailError::from_source("Comix", SourceError::Transport("Comix refused the request signature (keys of site build tmonyg)".into())).user_message();
        assert!(text.starts_with("Comix ") && !text.contains("signature") && !text.contains("keys"), "{text}");
    }

    #[test]
    fn http_103_is_ignored_others_are_not() {
        assert!(DetailError::Http(103).is_ignored());
        assert!(!DetailError::Http(500).is_ignored());
    }

    #[test]
    fn unknown_host_offline_and_online() {
        let off = DetailError::UnknownHost { online: false, host: "x".into() };
        assert_eq!(off.user_message(), "You're offline. Check your connection and try again.");
        let on = DetailError::UnknownHost { online: true, host: "example.org".into() };
        assert!(!on.user_message().contains("example.org"), "the host name stays in the log");
    }

    #[test]
    fn a_plain_failure_shows_its_sentence_and_can_be_retried() {
        let error = DetailError::Plain("Comix is not answering. Try again in a moment.".into());
        assert_eq!(error.user_message(), "Comix is not answering. Try again in a moment.");
        assert_eq!(error.refresh_message().as_deref(), Some("Comix is not answering. Try again in a moment."));
        assert!(!error.is_ignored());
    }

    #[test]
    fn refresh_messages_for_empty_lists_and_cancellation() {
        assert_eq!(DetailError::NoEpisodes.refresh_message().as_deref(), Some("No episodes found"));
        assert_eq!(DetailError::NoSeasons.refresh_message().as_deref(), Some("No seasons found"));
        assert_eq!(DetailError::Cancelled.refresh_message(), None);
        assert_eq!(DetailError::Http(500).refresh_message(), Some(DetailError::Http(500).user_message()));
    }

    #[test]
    fn source_not_installed_text() {
        let text = DetailError::SourceNotInstalled.user_message();
        assert!(!text.is_empty());
    }

    #[test]
    fn any_other_exception_is_the_generic_sentence_and_never_its_class_or_message() {
        for (class, message) in [("IOException", Some("broken pipe")), ("Exception", None), ("SQLiteException", Some("no such table: chapters"))] {
            let error = DetailError::Other { class: class.into(), message: message.map(str::to_owned) };
            assert_eq!(error.user_message(), "Something went wrong. Try again in a moment.");
        }
    }
}
