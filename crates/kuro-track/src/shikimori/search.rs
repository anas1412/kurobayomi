//! Shikimori catalogue record to search result (T-31a-009, `dto/SMEntry.kt`). A missing count is 0 here, so one
//! record without a count does not fail the whole search (the source's `!!` does, a recorded decision).

use super::dto::SmEntry;
use crate::model::TrackerId;
use crate::model::search::{AnimeTrackSearch, MangaTrackSearch};

/// The manga search result of a record. `base` is the site root, e.g. `https://shikimori.one`.
pub fn to_manga_search(base: &str, e: &SmEntry) -> MangaTrackSearch {
    let mut out = MangaTrackSearch::new(
        TrackerId::SHIKIMORI,
        e.id,
        e.name.clone(),
        format!("{base}{}", e.url),
    );
    out.total_chapters = e.chapters.unwrap_or(0);
    out.score = Some(e.score);
    out.cover_url = format!("{base}{}", e.image.preview);
    out.publishing_status = e.status.clone();
    out.publishing_type = e.kind.clone();
    out.start_date = e.aired_on.clone().unwrap_or_default();
    out
}

/// The anime search result of a record.
pub fn to_anime_search(base: &str, e: &SmEntry) -> AnimeTrackSearch {
    let mut out = AnimeTrackSearch::new(
        TrackerId::SHIKIMORI,
        e.id,
        e.name.clone(),
        format!("{base}{}", e.url),
    );
    out.total_episodes = e.episodes.unwrap_or(0);
    out.score = Some(e.score);
    out.cover_url = format!("{base}{}", e.image.preview);
    out.publishing_status = e.status.clone();
    out.publishing_type = e.kind.clone();
    out.start_date = e.aired_on.clone().unwrap_or_default();
    out
}
