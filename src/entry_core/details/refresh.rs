//! The notification a failed anime refresh shows (T-39b-021, T-39b-061). Upstream: `AnimeScreenModel.kt:350-363` (the
//! error messages). The rules of a refresh (what started it, the 10-minute guard, the auto-download, the single-flight
//! gate) are `refresh_rules`, shared with the manga page.

/// The notification a failed refresh shows (behaviour 19): the error text. A cancellation shows nothing, and neither do
/// the two empty-list errors: the list's empty state says them, with Retry.
pub fn refresh_snack(error: &crate::entry_core::errors::DetailError) -> Option<crate::entry_core::message::UserMessage> {
    use crate::entry_core::errors::DetailError;
    use crate::entry_core::message::UserMessage;
    match error {
        DetailError::Cancelled | DetailError::NoEpisodes | DetailError::NoSeasons => None,
        other => other.refresh_message().map(UserMessage::Plain),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_refresh_shows_its_text_and_a_cancelled_one_nothing() {
        use crate::entry_core::errors::DetailError;
        use crate::entry_core::message::text;
        assert_eq!(refresh_snack(&DetailError::NoEpisodes), None, "the empty list says it");
        assert_eq!(refresh_snack(&DetailError::NoSeasons), None, "the empty list says it");
        assert_eq!(refresh_snack(&DetailError::Cancelled), None);
        assert_eq!(text(&refresh_snack(&DetailError::Http(500)).expect("a snack")), DetailError::Http(500).user_message());
    }
}
