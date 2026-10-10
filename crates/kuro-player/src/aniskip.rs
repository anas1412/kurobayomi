//! AniSkip client and the AniList to MAL id lookup (area 45 `AniSkipApi.kt`, T-45b-020). Parsing is
//! pure and tested on fixtures. Any transport, status or decode failure is `None`, and nothing is
//! retried or shown to the user.

use std::future::Future;

use bytes::Bytes;
use kuro_net::{Body, NetworkClient};
use serde_json::Value;

use crate::types::{ChapterType, TimeStamp};

const SKIP_TIMES_URL: &str = "https://api.aniskip.com/v2/skip-times";
const ANILIST_URL: &str = "https://graphql.anilist.co";

/// The skip-times GET for one episode. `episode` is the whole episode number (the Kotlin truncates
/// the stored `Double`) and `episode_length` is the player duration in whole seconds.
pub fn skip_times_url(mal_id: i64, episode: i32, episode_length: i64) -> String {
    format!(
        "{SKIP_TIMES_URL}/{mal_id}/{episode}?types[]=ed&types[]=mixed-ed&types[]=mixed-op&types[]=op&types[]=recap&episodeLength={episode_length}"
    )
}

/// Decodes a skip-times body. `found: false`, a missing `results` list, or any malformed item gives
/// `None`. An unknown `skipType` (for example `mixed-ed`) is kept as `Other` with its interval, so
/// one new type does not drop the whole response (E-04).
pub fn parse_skip_times(body: &str) -> Option<Vec<TimeStamp>> {
    let value: Value = serde_json::from_str(body).ok()?;
    if value.get("found")?.as_bool() != Some(true) {
        return None;
    }
    value
        .get("results")?
        .as_array()?
        .iter()
        .map(parse_stamp)
        .collect()
}

fn parse_stamp(item: &Value) -> Option<TimeStamp> {
    let interval = item.get("interval")?;
    let start = interval.get("startTime")?.as_f64()?;
    let end = interval.get("endTime")?.as_f64()?;
    let (name, kind) = match item.get("skipType")?.as_str()? {
        "op" => ("Opening", ChapterType::Opening),
        "ed" => ("Ending", ChapterType::Ending),
        "recap" => ("Recap", ChapterType::Recap),
        "mixed-op" => ("Mixed-op", ChapterType::MixedOp),
        _ => ("", ChapterType::Other),
    };
    Some(TimeStamp {
        start,
        end,
        name: name.to_owned(),
        kind,
    })
}

/// The AniList GraphQL body that asks for an entry's MAL id. The id is interpolated, as the Kotlin
/// does, not sent as a variable.
pub fn anilist_body(anilist_id: i64) -> String {
    format!(r#"{{"query":"query{{\n Media(id:{anilist_id}){{idMal}}\n }}"}}"#)
}

/// The MAL id from an AniList response. `null` or a missing id is `None` (the Kotlin's `0`).
pub fn parse_mal_id(body: &str) -> Option<i64> {
    let value: Value = serde_json::from_str(body).ok()?;
    value.pointer("/data/Media/idMal")?.as_i64()
}

/// A tracker entry of the anime, reduced to what the MAL id lookup needs (T-45b-021).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackerRef {
    /// MyAnimeList: the remote id is the MAL id.
    MyAnimeList(i64),
    /// AniList: the MAL id comes from the lookup.
    AniList(i64),
    /// Any other tracker has no MAL id.
    Other,
}

/// The MAL id for AniSkip (E-03 fix). The first track whose tracker gives a non-zero MAL id wins:
/// a MAL track gives its remote id, an AniList track asks `anilist_lookup`, and other trackers are
/// passed over. `None` when no track resolves, so AniSkip is not called.
pub fn resolve_mal_id(
    tracks: &[TrackerRef],
    mut anilist_lookup: impl FnMut(i64) -> Option<i64>,
) -> Option<i64> {
    tracks.iter().find_map(|track| {
        let id = match *track {
            TrackerRef::MyAnimeList(id) => Some(id),
            TrackerRef::AniList(id) => anilist_lookup(id),
            TrackerRef::Other => None,
        };
        id.filter(|id| *id != 0)
    })
}

/// The `episodeLength` of the skip query: the player duration in whole seconds. A missing, zero or
/// non-finite duration gives `None`, and AniSkip is not called (T-45b-021).
pub fn episode_length(duration: Option<f64>) -> Option<i64> {
    let secs = duration.filter(|d| d.is_finite() && *d > 0.0)?;
    Some(secs as i64)
}

/// Fetches the skip intervals of one episode. Any error gives `None`.
pub async fn fetch_skip_times(
    client: &NetworkClient,
    mal_id: i64,
    episode: i32,
    episode_length: i64,
) -> Option<Vec<TimeStamp>> {
    let url = skip_times_url(mal_id, episode, episode_length);
    let response = client.get(&url).await.ok()?.await_success().ok()?;
    parse_skip_times(&response.text().await.ok()?)
}

/// Looks up the MAL id of an AniList entry. Any error gives `None`.
pub async fn fetch_mal_id(client: &NetworkClient, anilist_id: i64) -> Option<i64> {
    let body = Body::Bytes {
        content_type: "application/json; charset=utf-8".to_owned(),
        data: Bytes::from(anilist_body(anilist_id)),
    };
    let response = client
        .post(ANILIST_URL, body)
        .await
        .ok()?
        .await_success()
        .ok()?;
    parse_mal_id(&response.text().await.ok()?)
}

/// The two AniSkip requests the lookup makes, so the orchestration is tested without a network.
/// `NetworkClient` is the real one.
pub trait SkipApi {
    fn mal_id_of(&self, anilist_id: i64) -> impl Future<Output = Option<i64>> + Send;
    fn skip_times(
        &self,
        mal_id: i64,
        episode: i32,
        episode_length: i64,
    ) -> impl Future<Output = Option<Vec<TimeStamp>>> + Send;
}

impl SkipApi for NetworkClient {
    fn mal_id_of(&self, anilist_id: i64) -> impl Future<Output = Option<i64>> + Send {
        fetch_mal_id(self, anilist_id)
    }

    fn skip_times(
        &self,
        mal_id: i64,
        episode: i32,
        episode_length: i64,
    ) -> impl Future<Output = Option<Vec<TimeStamp>>> + Send {
        fetch_skip_times(self, mal_id, episode, episode_length)
    }
}

/// Whether the lookup runs after `FILE_LOADED`: intro skip and AniSkip are on, and the video has
/// no chapters of its own unless `pref_disabled_ani_skip_chapters` is off (`PlayerActivity.fileLoaded`).
pub fn lookup_wanted(
    intro_skip: bool,
    aniskip: bool,
    disable_on_chapters: bool,
    has_chapters: bool,
) -> bool {
    intro_skip && aniskip && !(disable_on_chapters && has_chapters)
}

/// The AniSkip lookup of one episode (T-45b-021): the skip intervals for the first track that
/// gives a non-zero MAL id, from one skip-times request. Nothing is requested when the duration is
/// missing, and no AniSkip request follows a failed AniList lookup. The caller merges the result
/// into the chapters (`Session::apply_aniskip`).
pub async fn lookup(
    api: &impl SkipApi,
    tracks: &[TrackerRef],
    episode_number: f64,
    duration: Option<f64>,
) -> Option<Vec<TimeStamp>> {
    let length = episode_length(duration)?;
    let mut mal_id = None;
    for track in tracks {
        let id = match *track {
            TrackerRef::MyAnimeList(id) => Some(id),
            TrackerRef::AniList(id) => api.mal_id_of(id).await,
            TrackerRef::Other => None,
        };
        if let Some(id) = id.filter(|id| *id != 0) {
            mal_id = Some(id);
            break;
        }
    }
    api.skip_times(mal_id?, episode_number as i32, length).await
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIVE: &str = r#"{"found":true,"results":[{"interval":{"startTime":1387.996,"endTime":1500},"skipType":"ed","skipId":"b60b5bf4-7fc9-40e9-b178-71d2a4f0e6c9","episodeLength":1500.146},{"interval":{"startTime":28.783,"endTime":118.783},"skipType":"op","skipId":"bf98fb07-c8f8-4399-baa4-1579f03b7e90","episodeLength":1500.04}],"message":"Successfully found skip times","statusCode":200}"#;

    #[test]
    fn url_matches_the_api_template() {
        assert_eq!(
            skip_times_url(21, 1, 1500),
            "https://api.aniskip.com/v2/skip-times/21/1?types[]=ed&types[]=mixed-ed&types[]=mixed-op&types[]=op&types[]=recap&episodeLength=1500"
        );
    }

    #[test]
    fn t36_decodes_the_live_sample() {
        let stamps = parse_skip_times(LIVE).unwrap();
        assert_eq!(stamps.len(), 2);
        assert_eq!(stamps[0].name, "Ending");
        assert_eq!(stamps[0].kind, ChapterType::Ending);
        assert_eq!(stamps[0].start, 1387.996);
        assert_eq!(stamps[1].name, "Opening");
        assert_eq!(stamps[1].kind, ChapterType::Opening);
        assert_eq!(stamps[1].end, 118.783);
    }

    #[test]
    fn t37_not_found_is_none() {
        let body = r#"{"found":false,"results":null,"message":"Not found","statusCode":404}"#;
        assert_eq!(parse_skip_times(body), None);
    }

    #[test]
    fn t38_unknown_skip_type_is_other_and_keeps_the_interval() {
        let body = r#"{"found":true,"results":[{"interval":{"startTime":5,"endTime":9},"skipType":"mixed-ed"}]}"#;
        let stamps = parse_skip_times(body).unwrap();
        assert_eq!(stamps.len(), 1);
        assert_eq!(stamps[0].kind, ChapterType::Other);
        assert_eq!(stamps[0].name, "");
        assert_eq!((stamps[0].start, stamps[0].end), (5.0, 9.0));
    }

    #[test]
    fn malformed_body_is_none() {
        assert_eq!(parse_skip_times("<html>"), None);
        assert_eq!(parse_skip_times(r#"{"found":true}"#), None);
    }

    #[test]
    fn mal_id_from_anilist_body() {
        assert_eq!(parse_mal_id(r#"{"data":{"Media":{"idMal":21}}}"#), Some(21));
        assert_eq!(parse_mal_id(r#"{"data":{"Media":{"idMal":null}}}"#), None);
        assert_eq!(parse_mal_id("nope"), None);
    }

    #[test]
    fn the_first_track_that_resolves_gives_the_mal_id() {
        use TrackerRef::*;
        let never = |_| panic!("no lookup needed");
        assert_eq!(resolve_mal_id(&[Other, MyAnimeList(21)], never), Some(21));
        assert_eq!(
            resolve_mal_id(&[MyAnimeList(21), AniList(9)], never),
            Some(21)
        );
        assert_eq!(resolve_mal_id(&[Other], never), None);
        assert_eq!(resolve_mal_id(&[], never), None);
        // A zero MAL id or a lookup without an id passes on to the next track.
        assert_eq!(
            resolve_mal_id(&[MyAnimeList(0), AniList(5), MyAnimeList(7)], |_| None),
            Some(7)
        );
        assert_eq!(resolve_mal_id(&[AniList(5)], |id| Some(id * 2)), Some(10));
        assert_eq!(resolve_mal_id(&[AniList(5)], |_| Some(0)), None);
    }

    #[test]
    fn episode_length_is_whole_seconds_or_no_call() {
        assert_eq!(episode_length(Some(1500.9)), Some(1500));
        assert_eq!(episode_length(None), None);
        assert_eq!(episode_length(Some(0.0)), None);
        assert_eq!(episode_length(Some(f64::NAN)), None);
    }

    #[test]
    fn anilist_request_interpolates_the_id() {
        let body: Value = serde_json::from_str(&anilist_body(154587)).unwrap();
        assert_eq!(
            body["query"].as_str().unwrap(),
            "query{\n Media(id:154587){idMal}\n }"
        );
    }

    use std::sync::Mutex;

    #[derive(Default)]
    struct Fake {
        mal_of: Option<i64>,
        calls: Mutex<Vec<String>>,
    }

    impl SkipApi for Fake {
        fn mal_id_of(&self, anilist_id: i64) -> impl Future<Output = Option<i64>> + Send {
            self.calls.lock().unwrap().push(format!("anilist {anilist_id}"));
            std::future::ready(self.mal_of)
        }

        fn skip_times(
            &self,
            mal_id: i64,
            episode: i32,
            length: i64,
        ) -> impl Future<Output = Option<Vec<TimeStamp>>> + Send {
            self.calls
                .lock()
                .unwrap()
                .push(format!("skip {mal_id} {episode} {length}"));
            std::future::ready(parse_skip_times(LIVE))
        }
    }

    #[tokio::test]
    async fn lookup_queries_aniskip_once_for_the_first_resolved_track() {
        let api = Fake {
            mal_of: Some(21),
            ..Fake::default()
        };
        let tracks = [TrackerRef::Other, TrackerRef::AniList(9), TrackerRef::MyAnimeList(5)];
        let stamps = lookup(&api, &tracks, 3.0, Some(1500.9)).await.unwrap();
        assert_eq!(stamps.len(), 2);
        assert_eq!(*api.calls.lock().unwrap(), vec!["anilist 9", "skip 21 3 1500"]);
    }

    #[tokio::test]
    async fn lookup_makes_no_call_when_the_anilist_lookup_fails_or_the_duration_is_missing() {
        let api = Fake::default();
        assert_eq!(lookup(&api, &[TrackerRef::AniList(9)], 1.0, Some(100.0)).await, None);
        assert_eq!(*api.calls.lock().unwrap(), vec!["anilist 9"]);
        let api = Fake::default();
        assert_eq!(lookup(&api, &[TrackerRef::MyAnimeList(5)], 1.0, None).await, None);
        assert_eq!(lookup(&api, &[], 1.0, Some(100.0)).await, None);
        assert!(api.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn lookup_runs_only_with_intro_skip_and_aniskip_on_and_no_blocking_chapters() {
        assert!(lookup_wanted(true, true, true, false));
        assert!(lookup_wanted(true, true, false, true));
        assert!(!lookup_wanted(true, true, true, true));
        assert!(!lookup_wanted(false, true, false, false));
        assert!(!lookup_wanted(true, false, false, false));
    }
}
