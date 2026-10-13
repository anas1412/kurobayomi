//! Conversions between the source model and the database model: the update strategy both ways, and a stored manga entry
//! back to `SManga` (`Manga.kt` `toSManga`, EC-MR-07).

use kuro_data::entry::Entry;
use kuro_db::codecs::UpdateStrategy;
use kuro_source::model::manga::UNKNOWN;
use kuro_source::model::{SManga, UpdateStrategy as SourceUpdateStrategy};

pub(crate) fn to_db_strategy(strategy: SourceUpdateStrategy) -> UpdateStrategy {
    match strategy {
        SourceUpdateStrategy::AlwaysUpdate => UpdateStrategy::AlwaysUpdate,
        SourceUpdateStrategy::OnlyFetchOnce => UpdateStrategy::OnlyFetchOnce,
    }
}

pub(crate) fn to_source_strategy(strategy: UpdateStrategy) -> SourceUpdateStrategy {
    match strategy {
        UpdateStrategy::AlwaysUpdate => SourceUpdateStrategy::AlwaysUpdate,
        UpdateStrategy::OnlyFetchOnce => SourceUpdateStrategy::OnlyFetchOnce,
    }
}

/// `toSManga`: the source view of a stored row. The update strategy is copied, so `OnlyFetchOnce` survives the round
/// trip (EC-MR-07, the documented change from Kotlin).
pub fn to_s_manga(manga: &Entry) -> SManga {
    SManga {
        url: manga.url.clone(),
        title: manga.title.clone(),
        artist: manga.artist.clone(),
        author: manga.author.clone(),
        description: manga.description.clone(),
        genre: manga.genre.as_ref().map(|genres| genres.join(", ")),
        status: i32::try_from(manga.status).unwrap_or(UNKNOWN),
        thumbnail_url: manga.thumbnail_url.clone(),
        update_strategy: to_source_strategy(manga.update_strategy),
        initialized: manga.initialized,
    }
}

#[cfg(test)]
mod tests {
    use kuro_platform::kind::Kind;

    use super::*;

    /// Topic 11, vector 8 and EC-MR-07: `OnlyFetchOnce` survives a round trip through the database model.
    #[test]
    fn the_update_strategy_round_trips_through_a_stored_row() {
        let row = Entry { update_strategy: to_db_strategy(SourceUpdateStrategy::OnlyFetchOnce), ..Entry::create(Kind::Manga) };
        assert_eq!(row.update_strategy, UpdateStrategy::OnlyFetchOnce);
        assert_eq!(to_s_manga(&row).update_strategy, SourceUpdateStrategy::OnlyFetchOnce);
    }

    #[test]
    fn the_stored_genres_come_back_as_one_text_and_split_again() {
        let row = Entry { genre: Some(vec!["Action".into(), "Drama".into()]), ..Entry::create(Kind::Manga) };
        let back = to_s_manga(&row);
        assert_eq!(back.genre.as_deref(), Some("Action, Drama"));
        assert_eq!(back.genres(), Some(vec!["Action".to_owned(), "Drama".to_owned()]));
    }

    #[test]
    fn a_status_outside_the_range_of_the_source_reads_as_unknown() {
        let row = Entry { status: i64::MAX, ..Entry::create(Kind::Manga) };
        assert_eq!(to_s_manga(&row).status, UNKNOWN);
    }
}
