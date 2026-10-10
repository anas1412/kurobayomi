//! Wire tests of the Komga client against a local mock server: the paths, the Basic credential, the progress bodies
//! and the reads that follow a write (T-33a-061 to T-33a-063; E-38 to E-49).

use wiremock::matchers::{body_string, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::COMPLETED;
use super::READING;
use super::api::KomgaApi;
use super::auth::KomgaAuth;
use crate::model::{MangaTrack, TrackerId};
use crate::trackers::http::tracker_client;

const SERIES: &str = r#"{"metadata":{"status":"ENDED","title":"Title","summary":"Sum"}}"#;
/// booksCount 10, unread 4, read 6, last continuous 3, max 12.9.
const PROGRESS_V2: &str = r#"{"booksCount":10,"booksReadCount":6,"booksUnreadCount":4,"booksInProgressCount":0,"lastReadContinuousNumberSort":3.0,"maxNumberSort":12.9}"#;
const READLIST: &str = r#"{"name":"Reading list"}"#;
/// booksCount 5, all read.
const PROGRESS_V1: &str = r#"{"booksCount":5,"booksReadCount":5,"booksUnreadCount":0,"booksInProgressCount":0,"lastReadContinuousIndex":5}"#;

fn api() -> KomgaApi {
    KomgaApi::new(tracker_client().unwrap())
}

fn creds() -> KomgaAuth {
    KomgaAuth::new("u".into(), "p".into())
}

fn track(url: &str, last: f64) -> MangaTrack {
    MangaTrack {
        id: Some(1),
        manga_id: 1,
        tracker_id: TrackerId::KOMGA,
        remote_id: 0,
        library_id: None,
        title: "T".into(),
        last_chapter_read: last,
        total_chapters: 0,
        status: READING,
        score: 4.0,
        remote_url: url.into(),
        start_date: 0,
        finish_date: 0,
        private: false,
    }
}

async fn get(server: &MockServer, p: &str, body: &str) {
    Mock::given(method("GET"))
        .and(path(p))
        .and(header("authorization", "Basic dTpw"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_owned()))
        .mount(server)
        .await;
}

#[tokio::test]
async fn t_33a_071_e39_a_series_reads_the_v2_progress_and_its_status() {
    let server = MockServer::start().await;
    get(&server, "/api/v1/series/9", SERIES).await;
    get(
        &server,
        "/api/v2/series/9/read-progress/tachiyomi",
        PROGRESS_V2,
    )
    .await;
    let url = format!("{}/api/v1/series/9", server.uri());
    let found = api().get_track_search(&creds(), &url).await.unwrap();
    assert_eq!(found.title, "Title");
    assert_eq!(found.summary, "Sum");
    assert_eq!(found.total_chapters, 12);
    assert_eq!(found.last_chapter_read, 3.0);
    assert_eq!(found.status, READING);
    assert_eq!(found.cover_url, format!("{url}/thumbnail"));
}

#[tokio::test]
async fn t_33a_070_e38_a_readlist_reads_the_v1_progress_at_its_own_path() {
    let server = MockServer::start().await;
    get(&server, "/api/v1/readlists/5", READLIST).await;
    get(
        &server,
        "/api/v1/readlists/5/read-progress/tachiyomi",
        PROGRESS_V1,
    )
    .await;
    let url = format!("{}/api/v1/readlists/5", server.uri());
    let found = api().get_track_search(&creds(), &url).await.unwrap();
    assert_eq!(found.title, "Reading list");
    assert_eq!(found.total_chapters, 5);
    assert_eq!(found.status, COMPLETED);
}

#[tokio::test]
async fn t_33a_071_e40_a_stored_v2_series_url_parses_as_v2() {
    let server = MockServer::start().await;
    get(&server, "/api/v2/series/9", SERIES).await;
    get(
        &server,
        "/api/v2/series/9/read-progress/tachiyomi",
        PROGRESS_V2,
    )
    .await;
    let url = format!("{}/api/v2/series/9", server.uri());
    let found = api().get_track_search(&creds(), &url).await.unwrap();
    assert_eq!(found.last_chapter_read, 3.0);
}

#[tokio::test]
async fn t_33a_072_e46_a_series_update_sends_the_exact_body_then_reads_back() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/series/9/read-progress/tachiyomi"))
        .and(header("content-type", "application/json"))
        .and(body_string(r#"{"lastBookNumberSortRead":12.0}"#))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    get(&server, "/api/v1/series/9", SERIES).await;
    get(
        &server,
        "/api/v2/series/9/read-progress/tachiyomi",
        PROGRESS_V2,
    )
    .await;
    let url = format!("{}/api/v1/series/9", server.uri());
    let updated = api()
        .update_progress(&creds(), &track(&url, 12.0))
        .await
        .unwrap();
    assert_eq!(updated.last_chapter_read, 3.0);
    assert_eq!(updated.total_chapters, 12);
}

#[tokio::test]
async fn t_33a_072_e47_a_readlist_update_sends_a_truncated_count() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/readlists/5/read-progress/tachiyomi"))
        .and(body_string(r#"{"lastBookRead":2}"#))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    get(&server, "/api/v1/readlists/5", READLIST).await;
    get(
        &server,
        "/api/v1/readlists/5/read-progress/tachiyomi",
        PROGRESS_V1,
    )
    .await;
    let url = format!("{}/api/v1/readlists/5", server.uri());
    api()
        .update_progress(&creds(), &track(&url, 2.7))
        .await
        .unwrap();
}

#[tokio::test]
async fn t_33a_062_e49_no_credentials_is_unauthorized() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/series/9"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let url = format!("{}/api/v1/series/9", server.uri());
    let err = api()
        .get_track_search(&KomgaAuth::new(String::new(), String::new()), &url)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        crate::trackers::enhanced::EnhancedError::Unauthorized
    ));
}

#[tokio::test]
async fn t_33a_063_e91_refresh_keeps_the_local_score() {
    use crate::events::EventBus;
    use crate::fakes::MemSecrets;
    use crate::tracker::{MangaTracker, Tracker};
    use crate::trackers::komga::KomgaTracker;
    let server = MockServer::start().await;
    get(&server, "/api/v1/series/9", SERIES).await;
    get(
        &server,
        "/api/v2/series/9/read-progress/tachiyomi",
        PROGRESS_V2,
    )
    .await;
    let tracker = KomgaTracker::new(MemSecrets::new(), EventBus::default()).unwrap();
    tracker.save_credentials("u", "p").unwrap();
    let local = track(&format!("{}/api/v1/series/9", server.uri()), 1.0);
    let refreshed = tracker.refresh(&local).await.unwrap();
    assert_eq!(refreshed.score, 4.0);
    assert_eq!(refreshed.last_chapter_read, 3.0);
}
