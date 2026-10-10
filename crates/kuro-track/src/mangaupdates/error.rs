//! MangaUpdates errors. The display texts are the texts the user sees (the Kotlin `HttpException` and `Throwable`
//! messages).

use crate::error::TrackError;
use crate::messages::MsgKey;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MuError {
    #[error("Not authenticated with MangaUpdates")]
    NotAuthenticated,
    /// A non-2xx reply. The snippet is the start of the body with tokens redacted; it is never shown to the user.
    #[error("HTTP {status}")]
    Http { status: u16, body_snippet: String },
    #[error("Network error: {0}")]
    Transport(String),
    #[error("Unexpected response from MangaUpdates: {0}")]
    Decode(String),
    #[error("score index {0} is not in the list")]
    ScoreOutOfRange(usize),
}

impl From<reqwest::Error> for MuError {
    fn from(e: reqwest::Error) -> Self {
        MuError::Transport(e.to_string())
    }
}

impl From<serde_json::Error> for MuError {
    fn from(e: serde_json::Error) -> Self {
        MuError::Decode(e.to_string())
    }
}

impl From<MuError> for TrackError {
    /// The error text is the toast, unchanged.
    fn from(e: MuError) -> Self {
        TrackError::Key(MsgKey::Raw(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_32a_011_user_visible_texts_match_the_source() {
        assert_eq!(
            MuError::NotAuthenticated.to_string(),
            "Not authenticated with MangaUpdates"
        );
        let refused = MuError::Http {
            status: 401,
            body_snippet: String::new(),
        };
        assert_eq!(refused.to_string(), "HTTP 401");
        assert_eq!(
            TrackError::from(refused).to_string(),
            "HTTP 401",
            "the toast is the error text"
        );
    }

    #[test]
    fn t_32a_012_a_json_failure_is_a_decode_error() {
        let e = serde_json::from_str::<u8>("x").unwrap_err();
        assert!(matches!(MuError::from(e), MuError::Decode(_)));
    }
}
