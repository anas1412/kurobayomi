//! `DownloadError` (thiserror). Each variant has a message key for the UI and an English `Display` for logs.

use kuro_source::SourceError;

use crate::store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    /// The downloads folder cannot be created or written (FM-01). Holds the root path.
    #[error("Invalid location: {0}")]
    InvalidLocation(String),
    #[error("No pages found")]
    NoPages,
    /// Free space is below the minimum (FM-02).
    #[error("Couldn't download due to low storage space")]
    LowSpace { free: u64, need: u64 },
    /// The folder does not hold one finished file per page.
    #[error("chapter is incomplete: {0}")]
    Incomplete(String),
    /// Some pages failed. Each failed page was already reported on its own.
    #[error("{0} page(s) failed")]
    PagesFailed(usize),
    /// Raw source text is shown as is, as Kotlin does.
    #[error(transparent)]
    Source(#[from] SourceError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A chapter task panicked, or the downloader is gone.
    #[error("internal error: {0}")]
    Internal(String),
}

/// How a chapter failure is handled by the scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// The transport failed. The chapter goes back to the queue when the network returns.
    Network,
    /// Stored in the queue as ERROR until the user retries.
    Other,
}

impl DownloadError {
    /// Message key for the UI (`p1-behavior.md` section 15). `None` when the text is raw source or I/O text.
    pub fn key(&self) -> Option<&'static str> {
        match self {
            Self::InvalidLocation(_) => Some("invalid_location"),
            Self::NoPages => Some("page_list_empty_error"),
            Self::LowSpace { .. } => Some("download_insufficient_space"),
            _ => None,
        }
    }

    pub fn failure_kind(&self) -> FailureKind {
        match self {
            Self::Source(SourceError::Transport(_)) => FailureKind::Network,
            _ => FailureKind::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_cover_the_mapped_messages() {
        assert_eq!(DownloadError::NoPages.key(), Some("page_list_empty_error"));
        assert_eq!(DownloadError::LowSpace { free: 1, need: 2 }.key(), Some("download_insufficient_space"));
        assert_eq!(DownloadError::InvalidLocation("/x".into()).key(), Some("invalid_location"));
        assert_eq!(DownloadError::Source(SourceError::Http(500)).key(), None);
    }

    #[test]
    fn only_transport_failures_count_as_network() {
        assert_eq!(
            DownloadError::Source(SourceError::Transport("reset".into())).failure_kind(),
            FailureKind::Network
        );
        assert_eq!(DownloadError::Source(SourceError::Http(404)).failure_kind(), FailureKind::Other);
        assert_eq!(DownloadError::NoPages.failure_kind(), FailureKind::Other);
    }

    #[test]
    fn display_uses_the_english_text() {
        assert_eq!(DownloadError::NoPages.to_string(), "No pages found");
        assert_eq!(DownloadError::Source(SourceError::Http(404)).to_string(), "HTTP error 404");
    }
}
