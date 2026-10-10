//! Simkl search results as tracker search rows (T-32a-037, T-32a-038; `SimklSearch.kt`, B-3.14) and the stored form of
//! their tracking URL (fix D-SK-2, R-5.2).

use super::TRACKER_ID;
use super::api::SearchKind;
use super::dto::SimklSearchResult;
use crate::model::search::AnimeTrackSearch;

/// Poster host. The poster name is appended with `_m.webp` (medium size, B-3.14).
pub const POSTERS_URL: &str = "https://simkl.in/posters/";

/// The path type of a stored tracking URL: `/tv/123` gives `tv`. `None` when there is no type segment, which is what an
/// absolute URL gives (the Kotlin `substringAfter("/")` gives `""` there, fix D-SK-2).
pub fn path_type(tracking_url: &str) -> Option<&str> {
    let rest = tracking_url.strip_prefix('/')?;
    rest.split('/').next().filter(|s| !s.is_empty())
}

/// The stored tracking URL of a search result, `/<anime|tv|movies>/<simkl id>`. The first path segment of the absolute
/// `url` decides it when it is a known one (`movie` is stored as `movies`). Otherwise the search kind decides.
pub fn normalised_tracking_url(absolute: &str, simkl_id: i64, kind: SearchKind) -> String {
    let known = url::Url::parse(absolute).ok().and_then(|u| {
        u.path_segments()
            .and_then(|mut s| s.next().map(str::to_owned))
    });
    let segment = match known.as_deref() {
        Some("movie" | "movies") => "movies",
        Some("anime") => "anime",
        Some("tv") => "tv",
        _ => kind.stored_segment(),
    };
    format!("/{segment}/{simkl_id}")
}

/// Maps one result (B-3.14). Two intended changes from the source: a missing title is an empty string and not a
/// panic (D-SK-8), and the tracking URL is stored in its relative form (D-SK-2).
pub fn to_track_search(r: &SimklSearchResult, kind: SearchKind) -> AnimeTrackSearch {
    let title = r
        .title_romaji
        .clone()
        .or_else(|| r.title.clone())
        .unwrap_or_default();
    let tracking_url = normalised_tracking_url(&r.url, r.ids.simkl_id, kind);
    let mut out = AnimeTrackSearch::new(TRACKER_ID, r.ids.simkl_id, title, tracking_url);
    // The absolute URL is what the browser opens; the stored tracking URL is relative (T-32a-038).
    out.source_url = r.url.clone();
    out.total_episodes = r.ep_count.unwrap_or(1);
    out.cover_url = r
        .poster
        .as_ref()
        .map(|p| format!("{POSTERS_URL}{p}_m.webp"))
        .unwrap_or_default();
    // The prefix stays even when the list is empty (`Some(vec![])` gives "All titles:\n", as the source).
    out.summary = r
        .all_titles
        .as_ref()
        .map(|titles| format!("All titles:\n{}", titles.join("\n")))
        .unwrap_or_default();
    out.publishing_status = r.status.clone().unwrap_or_else(|| "ended".to_owned());
    out.publishing_type = r.kind.clone().unwrap_or_else(|| kind.path().to_owned());
    out.start_date = r.year.map(|y| y.to_string()).unwrap_or_default();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TrackerId;

    fn result(json: &str) -> SimklSearchResult {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn t_32a_038_an_absolute_anime_url_is_stored_as_the_relative_form() {
        assert_eq!(
            normalised_tracking_url(
                "https://simkl.com/anime/1234/some-slug",
                1234,
                SearchKind::Anime
            ),
            "/anime/1234"
        );
    }

    #[test]
    fn t_32a_038_a_movie_result_is_stored_under_movies() {
        assert_eq!(
            normalised_tracking_url("https://simkl.com/movie/9/x", 9, SearchKind::Movie),
            "/movies/9"
        );
        assert_eq!(
            normalised_tracking_url("https://simkl.com/movies/9/x", 9, SearchKind::Movie),
            "/movies/9"
        );
    }

    #[test]
    fn t_32a_038_an_unknown_segment_falls_back_to_the_search_kind() {
        assert_eq!(
            normalised_tracking_url("https://simkl.com/x/1", 1, SearchKind::Tv),
            "/tv/1"
        );
        assert_eq!(
            normalised_tracking_url("https://simkl.com/x/1", 1, SearchKind::Movie),
            "/movies/1"
        );
    }

    #[test]
    fn t_32a_038_path_type_reads_relative_urls_and_gives_none_for_absolute_ones() {
        assert_eq!(path_type("/tv/123"), Some("tv"));
        assert_eq!(path_type("/movies/9"), Some("movies"));
        assert_eq!(path_type("https://simkl.com/tv/123"), None);
        assert_eq!(path_type(""), None);
    }

    #[test]
    fn t_32a_037_the_fields_map_per_the_source() {
        let r = result(
            r#"{"ids":{"simkl_id":42},"title_romaji":"Romaji","title":"Title","ep_count":12,"poster":"abc",
                "all_titles":["A","B"],"url":"https://simkl.com/anime/42/x","status":"airing",
                "type":"anime","year":2020}"#,
        );
        let out = to_track_search(&r, SearchKind::Anime);
        assert_eq!(out.tracker_id, TrackerId::SIMKL);
        assert_eq!(out.remote_id, 42);
        assert_eq!(out.title, "Romaji");
        assert_eq!(out.total_episodes, 12);
        assert_eq!(out.cover_url, "https://simkl.in/posters/abc_m.webp");
        assert_eq!(out.summary, "All titles:\nA\nB");
        assert_eq!(out.publishing_status, "airing");
        assert_eq!(out.publishing_type, "anime");
        assert_eq!(out.start_date, "2020");
        assert_eq!(out.tracking_url, "/anime/42");
        assert_eq!(out.source_url, "https://simkl.com/anime/42/x");
    }

    #[test]
    fn t_32a_038_the_absolute_result_url_is_kept_for_open_in_browser() {
        let r = result(r#"{"ids":{"simkl_id":9},"url":"https://simkl.com/movie/9/slug"}"#);
        let out = to_track_search(&r, SearchKind::Movie);
        assert_eq!(out.tracking_url, "/movies/9", "stored form");
        assert_eq!(out.source_url, "https://simkl.com/movie/9/slug");
        assert_eq!(out.open_url(), "https://simkl.com/movie/9/slug");
        assert_eq!(out.to_track().remote_url, "/movies/9", "the track keeps the relative form");
    }

    #[test]
    fn t_32a_037_missing_titles_give_an_empty_title_and_defaults_apply() {
        let r = result(r#"{"ids":{"simkl_id":1},"url":"https://simkl.com/tv/1/x"}"#);
        let out = to_track_search(&r, SearchKind::Tv);
        assert_eq!(out.title, "");
        assert_eq!(out.total_episodes, 1);
        assert_eq!(out.publishing_status, "ended");
        assert_eq!(out.publishing_type, "tv");
        assert_eq!(out.summary, "");
        assert_eq!(out.cover_url, "");
        assert_eq!(out.start_date, "");
    }

    #[test]
    fn t_32a_037_an_empty_title_list_keeps_the_summary_prefix() {
        let r = result(r#"{"ids":{"simkl_id":1},"url":"u","all_titles":[]}"#);
        assert_eq!(
            to_track_search(&r, SearchKind::Anime).summary,
            "All titles:\n"
        );
    }
}
