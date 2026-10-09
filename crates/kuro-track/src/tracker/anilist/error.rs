//! Typed failures of the AniList tracker (`p1-rust-design.md` section 3).

use std::time::Duration;

use kuro_platform::secrets::SecretError;

#[derive(Debug, thiserror::Error)]
pub enum AniListError {
    #[error("Not authenticated with AniList")]
    NotAuthenticated,
    /// HTTP 401, or a stored token past its expiry. The token is cleared before this is returned.
    #[error("AniList session expired")]
    AuthExpired,
    #[error("AniList rate limit reached")]
    RateLimited { retry_after: Option<Duration> },
    #[error("HTTP error {0}")]
    Http(u16),
    #[error("AniList server error")]
    Transient,
    /// Messages of the GraphQL `errors` array, joined with `"; "`. Sent on HTTP 200 too.
    #[error("AniList error: {0}")]
    GraphQl(String),
    #[error("could not read AniList response")]
    Decode(#[source] serde_json::Error),
    #[error("no network")]
    Offline,
    #[error("entry not found on your AniList list")]
    NotInList,
    #[error("unknown AniList status {0}")]
    UnknownStatus(String),
    #[error("unknown AniList score format {0}")]
    UnknownScoreFormat(String),
    #[error("score index out of range")]
    ScoreOutOfRange,
    #[error(transparent)]
    Transport(reqwest::Error),
    #[error(transparent)]
    Secret(#[from] SecretError),
    #[error(transparent)]
    Prefs(#[from] kuro_prefs::PrefError),
    /// A failure of the shared credential state (`BaseTracker`).
    #[error(transparent)]
    Track(#[from] crate::error::TrackError),
}

impl From<AniListError> for crate::error::TrackError {
    fn from(e: AniListError) -> Self {
        match e {
            AniListError::Track(t) => t,
            AniListError::Prefs(p) => crate::error::TrackError::Prefs(p),
            other => crate::error::TrackError::Message(other.to_string()),
        }
    }
}

impl AniListError {
    /// Maps a transport failure. A failed connection is reported as [`AniListError::Offline`].
    pub fn from_transport(e: reqwest::Error) -> Self {
        if e.is_connect() {
            AniListError::Offline
        } else {
            AniListError::Transport(e)
        }
    }
}
