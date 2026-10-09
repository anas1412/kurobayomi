//! Conversions between the source model and the database model: `SManga` to the metadata path, `SManga` to a new
//! `Manga`, and `Manga` back to `SManga` (`Manga.kt` `toSManga`/`toDomainManga`, T-15a-036, EC-MR-02, EC-MR-07).

use kuro_data::manga::{Manga, MangaUpdate};
use kuro_db::codecs::UpdateStrategy;
use kuro_source::model::manga::{UNKNOWN, parse_genres};
use kuro_source::model::{SManga, UpdateStrategy as SourceUpdateStrategy};

use super::manga::SourceMetadata;

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

/// The metadata a source sent, for `Manga::copy_from`. A genre text that holds no genre is `Some(None)`, which clears
/// the stored genres (EC-MR-02). No genre text at all is `None`, which keeps them.
pub fn source_metadata(manga: &SManga) -> SourceMetadata {
    SourceMetadata {
        author: manga.author.clone(),
        artist: manga.artist.clone(),
        description: manga.description.clone(),
        genre: manga.genre.as_deref().map(|text| parse_genres(Some(text))),
        thumbnail_url: manga.thumbnail_url.clone(),
        status: i64::from(manga.status),
        update_strategy: to_db_strategy(manga.update_strategy),
        initialized: manga.initialized,
    }
}

/// `toDomainManga`: a new library row for a manga a source reported, for the source with `source_id`.
pub fn manga_from_source(source_id: i64, manga: &SManga) -> Manga {
    let mut row = Manga::create();
    row.source = source_id;
    row.url.clone_from(&manga.url);
    row.title.clone_from(&manga.title);
    row.artist.clone_from(&manga.artist);
    row.author.clone_from(&manga.author);
    row.description.clone_from(&manga.description);
    row.genre = parse_genres(manga.genre.as_deref());
    row.status = i64::from(manga.status);
    row.thumbnail_url.clone_from(&manga.thumbnail_url);
    row.update_strategy = to_db_strategy(manga.update_strategy);
    row.initialized = manga.initialized;
    row
}

/// `UpdateManga` (library refresh): the update for a remote detail, written with coalesce semantics, so a `None` field
/// keeps the stored value. The title is written only for a non-favorite with a non-empty remote title. The thumbnail is
/// written only when it is non-empty. Genres come from the remote genre text (`None` keeps them, EC-MR-03). An empty
/// artist is written as empty (EC-MR-04). `initialized` is always set. The cover timestamp is `now_ms` only when the
/// non-empty remote thumbnail differs from `stored_thumbnail`; an empty or unchanged thumbnail keeps it. Custom covers
/// are not modelled here yet (area:34), so a changed thumbnail always bumps the timestamp.
pub fn refresh_update(
    id: i64,
    favorite: bool,
    remote: &SManga,
    stored_thumbnail: Option<&str>,
    now_ms: i64,
) -> MangaUpdate {
    let thumbnail_url = remote.thumbnail_url.clone().filter(|url| !url.is_empty());
    let cover_changed = thumbnail_url.as_deref().is_some_and(|url| Some(url) != stored_thumbnail);
    MangaUpdate {
        title: (!favorite && !remote.title.is_empty()).then(|| remote.title.clone()),
        artist: remote.artist.clone(),
        author: remote.author.clone(),
        description: remote.description.clone(),
        genre: parse_genres(remote.genre.as_deref()),
        thumbnail_url,
        cover_last_modified: cover_changed.then_some(now_ms),
        status: Some(i64::from(remote.status)),
        update_strategy: Some(to_db_strategy(remote.update_strategy)),
        initialized: Some(true),
        ..MangaUpdate::new(id)
    }
}

/// `toSManga`: the source view of a stored row. The update strategy is copied, so `OnlyFetchOnce` survives the round
/// trip (EC-MR-07, the documented change from Kotlin).
pub fn to_s_manga(manga: &Manga) -> SManga {
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
    use super::*;
    use crate::entries::manga::MangaExt;

    fn stored() -> Manga {
        Manga {
            author: Some("A".into()),
            artist: Some("B".into()),
            description: Some("D".into()),
            genre: Some(vec!["x".into()]),
            thumbnail_url: Some("t".into()),
            initialized: true,
            ..Manga::create()
        }
    }

    /// Topic 11, vector 1: the metadata path with a blank genre text, an empty artist and no author.
    #[test]
    fn the_metadata_path_from_an_s_manga_matches_the_vector() {
        let remote = SManga {
            author: None,
            artist: Some(String::new()),
            description: None,
            genre: Some(String::new()),
            thumbnail_url: None,
            status: 2,
            update_strategy: SourceUpdateStrategy::OnlyFetchOnce,
            initialized: false,
            ..SManga::new("/m", "M")
        };
        let mut row = stored();
        row.copy_from(&source_metadata(&remote));
        assert_eq!(row.author.as_deref(), Some("A"));
        assert_eq!(row.artist.as_deref(), Some(""));
        assert_eq!(row.description.as_deref(), Some("D"));
        assert_eq!(row.genre, None);
        assert_eq!(row.thumbnail_url.as_deref(), Some("t"));
        assert_eq!(row.status, 2);
        assert_eq!(row.update_strategy, UpdateStrategy::OnlyFetchOnce);
        assert!(!row.initialized, "true && false is false on the metadata path (EC-MR-05)");
    }

    #[test]
    fn no_genre_text_keeps_the_stored_genres() {
        let remote = SManga::new("/m", "M");
        let mut row = stored();
        row.copy_from(&source_metadata(&remote));
        assert_eq!(row.genre, Some(vec!["x".to_owned()]));
    }

    #[test]
    fn a_genre_text_with_genres_replaces_them() {
        let remote = SManga { genre: Some("a, b".into()), ..SManga::new("/m", "M") };
        let mut row = stored();
        row.copy_from(&source_metadata(&remote));
        assert_eq!(row.genre, Some(vec!["a".to_owned(), "b".to_owned()]));
    }

    /// Topic 11, vector 8 and EC-MR-07: `OnlyFetchOnce` survives a round trip through the database model.
    #[test]
    fn the_update_strategy_round_trips_through_a_stored_row() {
        let source = SManga { update_strategy: SourceUpdateStrategy::OnlyFetchOnce, ..SManga::new("/m", "M") };
        let row = manga_from_source(7, &source);
        assert_eq!(row.source, 7);
        assert_eq!(row.update_strategy, UpdateStrategy::OnlyFetchOnce);
        assert_eq!(to_s_manga(&row).update_strategy, SourceUpdateStrategy::OnlyFetchOnce);
    }

    #[test]
    fn the_stored_genres_come_back_as_one_text_and_split_again() {
        let row = Manga { genre: Some(vec!["Action".into(), "Drama".into()]), ..Manga::create() };
        let back = to_s_manga(&row);
        assert_eq!(back.genre.as_deref(), Some("Action, Drama"));
        assert_eq!(back.genres(), Some(vec!["Action".to_owned(), "Drama".to_owned()]));
    }

    /// Topic 11, vector 2: the refresh path keeps the genres and writes the empty artist.
    #[test]
    fn the_refresh_path_coalesces_genres_and_writes_an_empty_artist() {
        let remote = SManga {
            artist: Some(String::new()),
            genre: Some(String::new()),
            status: 2,
            update_strategy: SourceUpdateStrategy::OnlyFetchOnce,
            ..SManga::new("/m", "Remote")
        };
        let mut row = stored();
        row.apply_update(&refresh_update(row.id, true, &remote, row.thumbnail_url.as_deref(), 0));
        assert_eq!(row.genre, Some(vec!["x".to_owned()]));
        assert_eq!(row.artist.as_deref(), Some(""));
        assert_eq!(row.title, "", "a favorite keeps its title");
        assert!(row.initialized);
    }

    #[test]
    fn the_refresh_title_is_written_only_for_a_non_favorite_with_a_title() {
        let remote = SManga::new("/m", "Remote");
        assert_eq!(refresh_update(1, false, &remote, None, 0).title.as_deref(), Some("Remote"));
        assert_eq!(refresh_update(1, true, &remote, None, 0).title, None);
        assert_eq!(refresh_update(1, false, &SManga::new("/m", ""), None, 0).title, None);
    }

    /// EC-MR-06: an empty thumbnail never clears the stored one.
    #[test]
    fn an_empty_thumbnail_is_not_written() {
        let remote = SManga { thumbnail_url: Some(String::new()), ..SManga::new("/m", "M") };
        let mut row = stored();
        row.apply_update(&refresh_update(row.id, false, &remote, row.thumbnail_url.as_deref(), 9));
        assert_eq!(row.thumbnail_url.as_deref(), Some("t"));
    }

    /// Behaviour 11: a changed non-empty thumbnail takes `now`.
    #[test]
    fn a_changed_thumbnail_takes_the_refresh_time() {
        let remote = SManga { thumbnail_url: Some("new".into()), ..SManga::new("/m", "M") };
        assert_eq!(refresh_update(1, false, &remote, Some("old"), 42).cover_last_modified, Some(42));
        assert_eq!(refresh_update(1, false, &remote, None, 42).cover_last_modified, Some(42));
    }

    /// Behaviour 11: an unchanged thumbnail keeps the stored cover timestamp.
    #[test]
    fn an_unchanged_thumbnail_keeps_the_cover_timestamp() {
        let remote = SManga { thumbnail_url: Some("same".into()), ..SManga::new("/m", "M") };
        assert_eq!(refresh_update(1, false, &remote, Some("same"), 42).cover_last_modified, None);
    }

    /// Behaviour 11: an empty or missing remote thumbnail never changes the cover, even against a stored one.
    #[test]
    fn an_empty_thumbnail_keeps_the_cover_timestamp() {
        for thumbnail in [Some(String::new()), None] {
            let remote = SManga { thumbnail_url: thumbnail, ..SManga::new("/m", "M") };
            let update = refresh_update(1, false, &remote, Some("old"), 42);
            assert_eq!(update.cover_last_modified, None);
            assert_eq!(update.thumbnail_url, None);
        }
    }

    #[test]
    fn a_status_outside_the_range_of_the_source_reads_as_unknown() {
        let row = Manga { status: i64::MAX, ..Manga::create() };
        assert_eq!(to_s_manga(&row).status, UNKNOWN);
    }
}
