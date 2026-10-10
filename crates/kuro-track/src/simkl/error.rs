//! Simkl errors. The display texts are the texts the user sees (the Kotlin `Exception` messages).

use crate::error::TrackError;
use crate::messages::MsgKey;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SimklError {
    #[error("Not authenticated with Simkl")]
    NotAuthenticated,
    /// A non-2xx reply. The snippet is the start of the body with tokens redacted; it is never shown to the user.
    #[error("HTTP {status}")]
    Http { status: u16, body_snippet: String },
    #[error("Network error: {0}")]
    Transport(String),
    #[error("Unexpected response from Simkl: {0}")]
    Decode(String),
    #[error("Unexpected response from Simkl: missing {0}")]
    MissingField(&'static str),
    #[error("Unknown type: {0}")]
    UnknownMediaType(String),
    #[error("Unknown status: {0}")]
    UnknownStatus(i64),
    #[error("Unknown status: {0}")]
    UnknownWireStatus(String),
}

impl From<reqwest::Error> for SimklError {
    fn from(e: reqwest::Error) -> Self {
        SimklError::Transport(e.to_string())
    }
}

impl From<serde_json::Error> for SimklError {
    fn from(e: serde_json::Error) -> Self {
        SimklError::Decode(e.to_string())
    }
}

impl From<SimklError> for TrackError {
    /// The error text is the toast, unchanged.
    fn from(e: SimklError) -> Self {
        TrackError::Key(MsgKey::Raw(e.to_string()))
    }
}
