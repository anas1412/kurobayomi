//! Simkl library sequences: find in the list (B-3.5), the mapping of a sync item (B-3.6), and the write sequence of an
//! update (B-3.8: progress, list status, rating). The functions take the API, so the tracker and the tests share them.

use super::TRACKER_ID;
use super::api::{MediaKind, SimklApi};
use super::dto::SimklSyncItem;
use super::error::SimklError;
use super::search::path_type;
use super::status;
use crate::model::AnimeTrack;

/// The list a track's writes go to: `movies` for a movie, `shows` otherwise (B-3.8 step 1). A movie's stored URL is
/// `/movies/<id>` (see `search::normalised_tracking_url`).
pub fn media_kind(t: &AnimeTrack) -> MediaKind {
    match path_type(&t.remote_url) {
        Some("movies" | "movie") => MediaKind::Movies,
        _ => MediaKind::Shows,
    }
}

/// Finds the track in the user's list (`findLibAnime`, B-3.5 with fix D-SK-1). `None` when the title is not listed.
///
/// The Kotlin passes the URL segment `shows` to the parser, which rejects it (D-SK-1). Here the URL and the response key
/// are chosen separately, so a TV show binds.
pub async fn find_lib_anime(
    api: &SimklApi,
    t: &AnimeTrack,
) -> Result<Option<AnimeTrack>, SimklError> {
    let Some(watched) = api.find_watched(t.remote_id).await? else {
        return Ok(None);
    };
    let (Some(true), Some(last_watched), Some(list)) =
        (watched.result, watched.last_watched, watched.list)
    else {
        return Ok(None);
    };
    // Checked before any all-items request, so no URL with an empty type segment is ever built.
    let (path, key, want) = match path_type(&t.remote_url).unwrap_or("") {
        "tv" => ("shows", "tv", "show"),
        "anime" => ("anime", "anime", "show"),
        "movies" => ("movies", "movies", "movie"),
        other => return Err(SimklError::UnknownMediaType(other.to_owned())),
    };
    let all = api.all_items(path, &list, &last_watched).await?;
    let items = match key {
        "tv" => all.tv,
        "movies" => all.movies,
        _ => all.anime,
    };
    let found = items
        .unwrap_or_default()
        .iter()
        .find(|item| {
            item.result(want)
                .is_some_and(|r| r.ids.simkl == t.remote_id)
        })
        .map(|item| to_track(item, want, key, &list));
    found.transpose()
}

/// Maps a sync item to a track (`toAnimeTrack`, B-3.6). Missing counts are `MissingField`, not a panic (fix of `!!`).
/// `type_seg` is the stored path segment (`tv`, `anime` or `movies`).
pub fn to_track(
    item: &SimklSyncItem,
    want: &str,
    type_seg: &str,
    list: &str,
) -> Result<AnimeTrack, SimklError> {
    let result = item
        .result(want)
        .ok_or(SimklError::MissingField("show or movie"))?;
    let (total_episodes, last_episode_seen) = if want == "movie" {
        (1, if list == "completed" { 1.0 } else { 0.0 })
    } else {
        (
            item.total_episodes_count
                .ok_or(SimklError::MissingField("total_episodes_count"))?,
            item.watched_episodes_count
                .ok_or(SimklError::MissingField("watched_episodes_count"))?,
        )
    };
    Ok(AnimeTrack {
        id: None,
        anime_id: 0,
        tracker_id: TRACKER_ID,
        remote_id: result.ids.simkl,
        library_id: None,
        title: result.title.clone(),
        last_episode_seen,
        total_episodes,
        status: status::from_wire(list)?,
        score: item.user_rating.map_or(0.0, f64::from),
        remote_url: format!("/{type_seg}/{}", result.ids.simkl),
        start_date: 0,
        finish_date: 0,
        private: false,
    })
}

/// The write sequence of an update (`updateLibAnime`, B-3.8): progress (shows only), list status, then rating. Each is
/// one request, in this order. The status is checked first, so an unknown status sends nothing (fix D-SK-7, TST-29).
pub async fn push(api: &SimklApi, t: &AnimeTrack) -> Result<(), SimklError> {
    let wire = status::to_wire(t.status)?;
    let kind = media_kind(t);
    if kind == MediaKind::Shows {
        api.history_remove(t.remote_id).await?;
        api.history_add(t.remote_id, episodes_seen(t)).await?;
    }
    api.add_to_list(kind, t.remote_id, wire).await?;
    rate(api, kind, t).await
}

/// Only the list status is sent on add, with no progress and no rating (B-3.9).
pub async fn add(api: &SimklApi, t: &AnimeTrack) -> Result<(), SimklError> {
    let wire = status::to_wire(t.status)?;
    api.add_to_list(media_kind(t), t.remote_id, wire).await
}

/// Whole episodes watched, truncated toward zero (`toInt()`, TST-35).
fn episodes_seen(t: &AnimeTrack) -> u32 {
    t.last_episode_seen.trunc().max(0.0) as u32
}

/// Zero clears the rating. A negative score (the search default, -1.0) is never sent (TST-33).
async fn rate(api: &SimklApi, kind: MediaKind, t: &AnimeTrack) -> Result<(), SimklError> {
    if t.score < 0.0 {
        tracing::warn!("skipping a Simkl rating for a negative score");
        return Ok(());
    }
    if t.score == 0.0 {
        api.rating_remove(kind, t.remote_id).await
    } else {
        api.rating_set(kind, t.remote_id, t.score.trunc() as i32)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(json: &str) -> SimklSyncItem {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn t_32a_061_a_completed_movie_counts_as_one_of_one() {
        let it = item(r#"{"movie":{"title":"M","ids":{"simkl":9}},"user_rating":7}"#);
        let t = to_track(&it, "movie", "movies", "completed").unwrap();
        assert_eq!((t.total_episodes, t.last_episode_seen), (1, 1.0));
        assert_eq!(t.status, status::COMPLETED);
        assert_eq!(t.score, 7.0);
        assert_eq!(t.remote_url, "/movies/9");
        assert_eq!(t.remote_id, 9);
    }

    #[test]
    fn t_32a_061_a_movie_that_is_not_completed_has_zero_seen_and_needs_no_counts() {
        let it = item(r#"{"movie":{"title":"M","ids":{"simkl":9}}}"#);
        let t = to_track(&it, "movie", "movies", "plantowatch").unwrap();
        assert_eq!((t.total_episodes, t.last_episode_seen), (1, 0.0));
        assert_eq!(t.status, status::PLAN_TO_WATCH);
        assert_eq!(t.score, 0.0, "no rating is zero");
    }

    #[test]
    fn t_32a_061_a_show_with_a_missing_count_is_a_missing_field_error_not_a_panic() {
        let no_total = item(
            r#"{"show":{"title":"S","ids":{"simkl":5}},"total_episodes_count":null,"watched_episodes_count":3.0}"#,
        );
        assert!(matches!(
            to_track(&no_total, "show", "tv", "watching"),
            Err(SimklError::MissingField("total_episodes_count"))
        ));
        let no_seen = item(
            r#"{"show":{"title":"S","ids":{"simkl":5}},"total_episodes_count":12}"#,
        );
        assert!(matches!(
            to_track(&no_seen, "show", "tv", "watching"),
            Err(SimklError::MissingField("watched_episodes_count"))
        ));
    }

    #[test]
    fn t_32a_061_a_show_maps_its_counts_url_and_status() {
        let it = item(
            r#"{"show":{"title":"S","ids":{"simkl":5}},"total_episodes_count":12,"watched_episodes_count":3.0,"user_rating":8}"#,
        );
        let t = to_track(&it, "show", "anime", "watching").unwrap();
        assert_eq!((t.total_episodes, t.last_episode_seen), (12, 3.0));
        assert_eq!(t.remote_url, "/anime/5");
        assert_eq!(t.status, status::WATCHING);
        assert_eq!(t.title, "S");
    }

    #[test]
    fn t_32a_061_an_item_without_the_wanted_part_is_a_missing_field_error() {
        let it = item(r#"{"movie":{"title":"M","ids":{"simkl":9}}}"#);
        assert!(matches!(
            to_track(&it, "show", "tv", "watching"),
            Err(SimklError::MissingField("show or movie"))
        ));
    }
}
