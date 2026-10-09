//! The error-to-message table of a remote listing (T-06a-043, `ExceptionFormatter.kt`). The source
//! layer turns each failure into a [`SourceFailure`], and this module turns that into the text shown.

use crate::error::SourceError;
use crate::pager::PagerError;

/// A failure of a source call, in the kinds the message table tells apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceFailure {
    /// The source answered with this HTTP status.
    Http(u16),
    /// The host name did not resolve. `online` is whether the device had a connection when it failed.
    UnknownHost { host: String, online: bool },
    /// Any other failure, named by its kind (for example `IOException`). `message` is `None` when the
    /// failure carries no text.
    Named { kind: String, message: Option<String> },
    /// The source is not installed (`SourceNotInstalled`): it has no listings to show.
    NotInstalled,
}

impl SourceFailure {
    /// The table row of a source error. `online` is whether the device had a connection when the call failed, and
    /// only the unknown-host row depends on it. `None` for the kinds the table has no row for (they use `Display`).
    pub fn from_error(error: &SourceError, online: bool) -> Option<Self> {
        match error {
            SourceError::Http(code) => Some(Self::Http(*code)),
            SourceError::UnknownHost { host } => Some(Self::UnknownHost { host: host.clone(), online }),
            SourceError::NotInstalled(_) => Some(Self::NotInstalled),
            _ => None,
        }
    }
}

/// The text shown for one failure of a listing.
pub fn failure_message(failure: &SourceFailure) -> String {
    match failure {
        SourceFailure::NotInstalled => "Source not found".to_owned(),
        SourceFailure::Http(code) => format!("HTTP {code}, check website in WebView"),
        SourceFailure::UnknownHost { online: false, .. } => "No Internet connection".to_owned(),
        SourceFailure::UnknownHost { host, online: true } => format!("Couldn't reach {host}"),
        SourceFailure::Named { kind, message } => match (kind.as_str(), message) {
            // Plain exceptions show only their message, and only their kind when there is none.
            ("Exception" | "IOException", Some(text)) => text.clone(),
            ("Exception" | "IOException", None) => kind.clone(),
            (_, Some(text)) => format!("{kind}: {text}"),
            (_, None) => kind.clone(),
        },
    }
}

/// The text shown when a listing page fails: `NoResults` is "No results found", and a source failure
/// uses [`failure_message`].
pub fn error_message(error: &PagerError<SourceFailure>) -> String {
    match error {
        PagerError::NoResults => "No results found".to_owned(),
        PagerError::Source(failure) => failure_message(failure),
    }
}

/// The text shown when a source call fails outside a listing (`ExceptionFormatter.formattedMessage`). The HTTP,
/// offline and unknown-host texts match the English strings `exception_http`, `exception_offline` and
/// `exception_unknown_host`. `online` is whether the device had a connection when the call failed.
pub fn source_error_message(error: &SourceError, online: bool) -> String {
    if let Some(failure) = SourceFailure::from_error(error, online) {
        return failure_message(&failure);
    }
    match error {
        SourceError::EmptyPageList => "No pages found".to_owned(),
        SourceError::Transport(_) if !online => "No Internet connection".to_owned(),
        SourceError::Transport(text) => text.clone(),
        other => other.to_string(),
    }
}

/// The text for a chapter list that came back empty: "No results found" in browse (`no_results_found`), and "No
/// chapters found" in library updates (`no_chapters_error`).
pub fn no_chapters_message(in_library_updates: bool) -> &'static str {
    if in_library_updates { "No chapters found" } else { "No results found" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(kind: &str, message: Option<&str>) -> SourceFailure {
        SourceFailure::Named { kind: kind.to_owned(), message: message.map(str::to_owned) }
    }

    #[test]
    fn each_message_of_the_table() {
        let cases: [(SourceFailure, &str); 9] = [
            (SourceFailure::Http(404), "HTTP 404, check website in WebView"),
            (SourceFailure::Http(503), "HTTP 503, check website in WebView"),
            (SourceFailure::UnknownHost { host: "x.example".into(), online: false }, "No Internet connection"),
            (SourceFailure::UnknownHost { host: "x.example".into(), online: true }, "Couldn't reach x.example"),
            (named("IOException", Some("socket closed")), "socket closed"),
            (named("Exception", Some("boom")), "boom"),
            (named("IOException", None), "IOException"),
            (named("TimeoutException", Some("slow")), "TimeoutException: slow"),
            (named("SocketException", None), "SocketException"),
        ];
        for (failure, expected) in cases {
            assert_eq!(failure_message(&failure), expected, "{failure:?}");
        }
    }

    #[test]
    fn an_uninstalled_source_reads_source_not_found() {
        assert_eq!(failure_message(&SourceFailure::NotInstalled), "Source not found");
        assert_eq!(error_message(&PagerError::Source(SourceFailure::NotInstalled)), "Source not found");
    }

    #[test]
    fn no_results_is_its_own_message_and_a_source_error_uses_the_table() {
        assert_eq!(error_message(&PagerError::NoResults), "No results found");
        assert_eq!(error_message(&PagerError::Source(SourceFailure::Http(500))), "HTTP 500, check website in WebView");
    }

    #[test]
    fn source_errors_outside_a_listing_use_the_english_strings() {
        assert_eq!(source_error_message(&SourceError::Http(404), true), "HTTP 404, check website in WebView");
        assert_eq!(source_error_message(&SourceError::Transport("reset".into()), false), "No Internet connection");
        assert_eq!(source_error_message(&SourceError::Transport("reset".into()), true), "reset");
        assert_eq!(source_error_message(&SourceError::EmptyPageList, true), "No pages found");
        assert_eq!(source_error_message(&SourceError::NotInstalled(7), true), "Source not found");
    }

    #[test]
    fn an_unknown_host_is_offline_text_when_down_and_names_the_host_otherwise() {
        let error = SourceError::UnknownHost { host: "x.example".into() };
        assert_eq!(source_error_message(&error, false), "No Internet connection");
        assert_eq!(source_error_message(&error, true), "Couldn't reach x.example");
        assert_eq!(
            SourceFailure::from_error(&error, true),
            Some(SourceFailure::UnknownHost { host: "x.example".into(), online: true })
        );
        assert_eq!(SourceFailure::from_error(&SourceError::Cancelled, true), None);
    }

    #[test]
    fn an_empty_chapter_list_says_no_results_in_browse_and_no_chapters_in_updates() {
        assert_eq!(no_chapters_message(false), "No results found");
        assert_eq!(no_chapters_message(true), "No chapters found");
    }
}
