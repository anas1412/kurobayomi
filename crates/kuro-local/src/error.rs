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
    /// `ComicInfo.xml` is malformed, has the wrong root, or is empty (behavior 19.4, E-CI-7, E-CI-8).
    #[error("invalid ComicInfo.xml: {0}")]
    Xml(String),
    /// `chapters.json`, `episodes.json` or `details.json` does not decode (area:14 section 20).
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl LocalSourceError {
    /// A title folder that is not there (a drive that is not mounted, a folder moved away). It is an error, not an empty
    /// list, so a library update keeps the title's chapters or episodes and their progress until the folder is back.
    pub(crate) fn folder_missing() -> Self {
        Self::Io(io::Error::new(io::ErrorKind::NotFound, "the title folder is missing"))
    }
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
