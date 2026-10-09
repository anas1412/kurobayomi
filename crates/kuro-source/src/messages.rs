//! The error-to-message table of a remote listing (T-06a-043, `ExceptionFormatter.kt`). The source
//! layer turns each failure into a [`SourceFailure`], and this module turns that into the text shown.

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
}
