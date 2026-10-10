//! Shikimori errors (`plans/31-tracker-shikimori-bangumi/p1-rust-design.md` section 10). The display texts are the
//! texts the user sees, and they convert to `TrackError` unchanged.

use kuro_platform::secrets::SecretError;

use crate::error::TrackError;
use crate::messages::MsgKey;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShikimoriError {
    #[error("Not authenticated with Shikimori")]
    NotAuthenticated,
    /// The refresh was refused (400 or 401), or the stored login has no refresh token (D1, D3).
    #[error("Shikimori session has expired, log in again")]
    AuthExpired,
    #[error("HTTP error {0}")]
    Http(u16),
    #[error("{0}")]
    Transport(String),
    #[error("Failed to decode Shikimori response: {0}")]
    Decode(String),
    #[error("Unknown status: {0}")]
    UnknownStatus(String),
    /// Two library rows for one title. Manga and anime share the text (D13).
    #[error("Too many entries in response")]
    TooManyEntries,
    #[error("Could not find {0}")]
    NotFound(&'static str),
    /// A track with no library-entry id cannot be deleted remotely (D18).
    #[error("Missing library id")]
    NoLibraryId,
    /// No client id and secret in the user's configuration (D-OPEN-1).
    #[error("Shikimori is not configured")]
    NotConfigured,
    #[error("Secret storage is unavailable")]
    SecretStorage,
}

impl From<reqwest::Error> for ShikimoriError {
    fn from(e: reqwest::Error) -> Self {
        match e.status() {
            Some(code) => ShikimoriError::Http(code.as_u16()),
            None => ShikimoriError::Transport(e.to_string()),
        }
    }
}

impl From<serde_json::Error> for ShikimoriError {
    fn from(e: serde_json::Error) -> Self {
        ShikimoriError::Decode(e.to_string())
    }
}

impl From<SecretError> for ShikimoriError {
    fn from(_: SecretError) -> Self {
        ShikimoriError::SecretStorage
    }
}

impl From<ShikimoriError> for TrackError {
    fn from(e: ShikimoriError) -> Self {
        TrackError::Key(MsgKey::Raw(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_31a_005_user_visible_texts() {
        assert_eq!(
            ShikimoriError::NotAuthenticated.to_string(),
            "Not authenticated with Shikimori"
        );
        assert_eq!(ShikimoriError::Http(404).to_string(), "HTTP error 404");
        assert_eq!(
            ShikimoriError::TooManyEntries.to_string(),
            "Too many entries in response"
        );
        assert_eq!(ShikimoriError::NotFound("anime").to_string(), "Could not find anime");
    }
}
