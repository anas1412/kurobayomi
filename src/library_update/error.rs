//! Errors of one library pass. The user message is what the failure list and the error log show.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
    /// The source returned no chapters and is not a local source.
    NoChapters,
    /// The source returned no episodes and is not a local source.
    NoEpisodes,
    /// The entry's source is not installed (the stub of a missing source).
    SourceNotInstalled,
    /// The pass was stopped while the entry was running. Not a failure.
    Cancelled,
    /// Any other failure, shown verbatim. An empty message is shown as its Debug text.
    Other(String),
}

impl UpdateError {
    pub fn user_message(&self) -> String {
        match self {
            Self::NoChapters => "No chapters found".into(),
            Self::NoEpisodes => "No episodes found".into(),
            Self::SourceNotInstalled => "Source not found".into(),
            Self::Cancelled => "Cancelled".into(),
            Self::Other(message) if message.trim().is_empty() => format!("{self:?}"),
            Self::Other(message) => message.clone(),
        }
    }
}

impl From<kuro_db::DbError> for UpdateError {
    fn from(error: kuro_db::DbError) -> Self {
        Self::Other(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_errors_have_the_kotlin_texts() {
        assert_eq!(UpdateError::NoChapters.user_message(), "No chapters found");
        assert_eq!(UpdateError::NoEpisodes.user_message(), "No episodes found");
        assert_eq!(UpdateError::SourceNotInstalled.user_message(), "Source not found");
    }

    #[test]
    fn an_empty_message_becomes_its_debug_text() {
        // P-07: a null or empty message never reaches the failure list as an empty string.
        let text = UpdateError::Other(String::new()).user_message();
        assert!(!text.is_empty());
        assert_eq!(text, "Other(\"\")");
        assert_eq!(UpdateError::Other("timeout".into()).user_message(), "timeout");
    }
}
