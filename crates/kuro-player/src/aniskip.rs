//! AniSkip client and the AniList to MAL id lookup (area 45 `AniSkipApi.kt`, T-45b-020). Parsing is
//! pure and tested on fixtures. Any transport, status or decode failure is `None`, and nothing is
//! retried or shown to the user.

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
    fn decodes_the_live_sample() {
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
    fn not_found_is_none() {
        let body = r#"{"found":false,"results":null,"message":"Not found","statusCode":404}"#;
        assert_eq!(parse_skip_times(body), None);
    }

    #[test]
    fn unknown_skip_type_is_other_and_keeps_the_interval() {
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
    fn anilist_request_interpolates_the_id() {
        let body: Value = serde_json::from_str(&anilist_body(154587)).unwrap();
        assert_eq!(
            body["query"].as_str().unwrap(),
            "query{\n Media(id:154587){idMal}\n }"
        );
    }
}
