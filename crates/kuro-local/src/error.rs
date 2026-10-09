//! `LocalSourceError` for the local source (area:14 section 1).

use std::io;

use kuro_source::SourceError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LocalSourceError {
    /// Missing base folder, missing entry, or an unsafe URL (behavior 10.1, E-READ-1).
    #[error("Chapter not found")]
    ChapterNotFound,
    /// The entry exists but is neither a folder, an EPUB nor a manga archive (behavior 10.1).
    #[error("Invalid chapter format")]
    InvalidFormat,
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// The source traits return `SourceError`. A failed read is a transport-class failure there, since
/// `SourceError` has no local-file variant. Format errors never reach the traits (they belong to the
/// reader's inherent API), so they take the parse class.
impl From<LocalSourceError> for SourceError {
    fn from(error: LocalSourceError) -> Self {
        match error {
            LocalSourceError::Io(io) => SourceError::Transport(io.to_string()),
            other => SourceError::Parse(other.to_string()),
        }
    }
}
