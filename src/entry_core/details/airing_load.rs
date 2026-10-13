//! Where the airing countdown comes from (T-39b-036, T-39b-037). Upstream: `AniChartApi.kt` (`loadAiringTime`, the AniList
//! and MAL lookups) and the overwrite rule of `AnimeScreenModel.kt`. Pure: the decision of which tracker answers, the
//! reply parsers and the stored-data rule. The HTTP calls and the store write belong to the runner; the Simkl calendar is
//! not here, because its live schema has not been checked against a fixture (T-AIR-7, T-AIR-8 are open).

use serde_json::Value;

/// The `(episode, airing_at_seconds)` a lookup gives when nothing is known (`T-AIR-2`).
pub const UNKNOWN: (i32, i64) = (1, 0);

/// The trackers the countdown can read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tracker {
    AniList,
    MyAnimeList,
    Simkl,
    Other,
}

/// The service that answers, after the tracker items are checked in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    AniList,
    MyAnimeList,
    Simkl,
}

/// The first tracker item that is one of the three and has a track row answers (`T-AIR-1`). `items` is each tracker item
/// in order, with whether it has a track. `None` means no binding, which gives [`UNKNOWN`].
pub fn pick_source(items: &[(Tracker, bool)]) -> Option<Source> {
    items.iter().find_map(|(tracker, bound)| match (tracker, bound) {
        (Tracker::AniList, true) => Some(Source::AniList),
        (Tracker::MyAnimeList, true) => Some(Source::MyAnimeList),
        (Tracker::Simkl, true) => Some(Source::Simkl),
        _ => None,
    })
}

/// A completed entry keeps its stored airing data on an automatic update; a manual refresh always looks again (A-6).
pub fn keeps_stored(completed: bool, manual: bool) -> bool {
    completed && !manual
}

/// The episode and airing time out of an AniList `Media` reply. A `null` next episode, a missing field or a broken reply
/// gives [`UNKNOWN`] for the missing part: episode 1 and time 0 (`T-AIR-2`, `T-AIR-3`).
pub fn parse_airing(body: &str) -> (i32, i64) {
    let Ok(value) = serde_json::from_str::<Value>(body) else { return UNKNOWN };
    let Some(next) = value.pointer("/data/Media/nextAiringEpisode").filter(|v| v.is_object()) else { return UNKNOWN };
    let episode = next.get("episode").and_then(Value::as_i64).and_then(|e| i32::try_from(e).ok()).unwrap_or(UNKNOWN.0);
    let airing_at = next.get("airingAt").and_then(Value::as_i64).unwrap_or(UNKNOWN.1);
    (episode, airing_at)
}

/// The AniList id out of a MAL-id lookup reply, or 0 when there is none.
pub fn parse_anilist_id(body: &str) -> i64 {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| value.pointer("/data/Media/id").and_then(Value::as_i64))
        .unwrap_or(0)
}

/// A MAL lookup counts only when the server answered with a success status (`T-AIR-9`).
pub fn http_ok(status: u16) -> bool {
    (200..300).contains(&status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_bound_tracker_of_the_three_answers() {
        let items = [(Tracker::Other, true), (Tracker::MyAnimeList, false), (Tracker::Simkl, true), (Tracker::AniList, true)];
        assert_eq!(pick_source(&items), Some(Source::Simkl));
        assert_eq!(pick_source(&[(Tracker::AniList, false)]), None, "no track row: the unknown default");
        assert_eq!(pick_source(&[]), None);
    }

    #[test]
    fn a_completed_entry_keeps_its_data_except_on_a_manual_refresh() {
        assert!(keeps_stored(true, false));
        assert!(!keeps_stored(true, true));
        assert!(!keeps_stored(false, false));
    }

    #[test]
    fn a_null_next_episode_gives_the_unknown_default() {
        assert_eq!(parse_airing(r#"{"data":{"Media":{"nextAiringEpisode":null}}}"#), (1, 0));
        assert_eq!(parse_airing("not json"), (1, 0));
        assert_eq!(parse_airing(r#"{"data":{}}"#), (1, 0));
    }

    #[test]
    fn the_values_parse_from_the_reply() {
        let body = r#"{"data":{"Media":{"nextAiringEpisode":{"episode":7,"airingAt":1700000000}}}}"#;
        assert_eq!(parse_airing(body), (7, 1_700_000_000));
        let partial = r#"{"data":{"Media":{"nextAiringEpisode":{"episode":3}}}}"#;
        assert_eq!(parse_airing(partial), (3, 0), "a missing time is 0");
    }

    #[test]
    fn a_mal_lookup_reads_the_anilist_id_or_zero() {
        assert_eq!(parse_anilist_id(r#"{"data":{"Media":{"id":154587}}}"#), 154587);
        assert_eq!(parse_anilist_id(r#"{"data":{"Media":null}}"#), 0);
    }

    #[test]
    fn only_success_statuses_count() {
        assert!(http_ok(200));
        assert!(!http_ok(404));
        assert!(!http_ok(500));
    }
}
