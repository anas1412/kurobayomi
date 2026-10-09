//! Error text for the details page: `ExceptionFormatter.formattedMessage` and the chapter-fetch message.
//! Upstream: `app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt:16-38`.
//! Errors are typed here, so the mapping does not depend on runtime class names.

use kuro_i18n::{Arg, MsgKey, tr, trf};

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
    /// Any other error: `class` is the simple class name, `message` the exception message.
    Other {
        class: String,
        message: Option<String>,
    },
}

impl DetailError {
    /// `formattedMessage`: the text of a banner or toast for this error.
    pub fn user_message(&self) -> String {
        match self {
            Self::Http(code) => trf(MsgKey::exception_http, &[("arg1", Arg::Int(*code))]),
            Self::UnknownHost { online: false, .. } => tr(MsgKey::exception_offline),
            Self::UnknownHost { online: true, host } => {
                trf(MsgKey::exception_unknown_host, &[("arg1", Arg::Str(host))])
            }
            Self::NoChapters => tr(MsgKey::no_results_found),
            Self::SourceNotInstalled => tr(MsgKey::loader_not_implemented_error),
            Self::Other { class, message } => match class.as_str() {
                "Exception" | "IOException" => message.clone().unwrap_or_else(|| class.clone()),
                _ => format!("{class}: {}", message.as_deref().unwrap_or("null")),
            },
        }
    }

    /// The message for a failed chapter fetch. An empty list shows "No chapters found", not "No results found".
    pub fn chapter_fetch_message(&self) -> String {
        match self {
            Self::NoChapters => tr(MsgKey::no_chapters_error),
            other => other.user_message(),
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
    fn http_message_carries_the_code() {
        assert_eq!(
            DetailError::Http(404).user_message(),
            "HTTP 404, check website in WebView"
        );
    }

    #[test]
    fn http_103_is_ignored_others_are_not() {
        assert!(DetailError::Http(103).is_ignored());
        assert!(!DetailError::Http(500).is_ignored());
    }

    #[test]
    fn unknown_host_offline_and_online() {
        let off = DetailError::UnknownHost {
            online: false,
            host: "x".into(),
        };
        assert_eq!(off.user_message(), "No Internet connection");
        let on = DetailError::UnknownHost {
            online: true,
            host: "example.org".into(),
        };
        assert_eq!(on.user_message(), "Couldn't reach example.org");
    }

    #[test]
    fn no_chapters_differs_between_list_and_fetch() {
        assert_eq!(DetailError::NoChapters.user_message(), "No results found");
        assert_eq!(
            DetailError::NoChapters.chapter_fetch_message(),
            "No chapters found"
        );
        assert_eq!(
            DetailError::Http(500).chapter_fetch_message(),
            DetailError::Http(500).user_message()
        );
    }

    #[test]
    fn source_not_installed_text() {
        let text = DetailError::SourceNotInstalled.user_message();
        assert!(!text.is_empty());
    }

    #[test]
    fn generic_exception_branches() {
        let plain = DetailError::Other {
            class: "IOException".into(),
            message: Some("broken pipe".into()),
        };
        assert_eq!(plain.user_message(), "broken pipe");
        let no_msg = DetailError::Other {
            class: "Exception".into(),
            message: None,
        };
        assert_eq!(no_msg.user_message(), "Exception");
        let named = DetailError::Other {
            class: "ParseError".into(),
            message: Some("bad json".into()),
        };
        assert_eq!(named.user_message(), "ParseError: bad json");
        let named_null = DetailError::Other {
            class: "ParseError".into(),
            message: None,
        };
        assert_eq!(named_null.user_message(), "ParseError: null");
    }
}
