//! Wire tests of the Kavita client against a local mock server: login, the series read, the progress write and the
//! framework rules that reach the network (T-33a-057; E-01 to E-37, E-91 to E-93).

use std::sync::Arc;

use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::api::KavitaApi;
use super::auth::KavitaAuth;
use super::{COMPLETED, KavitaTracker, READING, UNREAD};
use crate::model::{MangaTrack, TrackerId};
use crate::tracker::Tracker;
use crate::trackers::enhanced::{EntryRef, MemFlags};
use crate::trackers::http::tracker_client;
use crate::trackers::sources::memory::MemDirectory;

const KEY: &str = "K1";
const TOKEN_BODY: &str = r#"{"username":"u","token":"tok","apiKey":"k"}"#;
const SERIES_OK: &str =
    r#"{"id":12,"name":"Berserk","pages":10,"pagesRead":4,"format":3,"libraryId":1}"#;
const VOLUMES_OK: &str = r#"[{"chapters":[{"number":"1"},{"number":"2"},{"number":"10.5"}]}]"#;

fn dir_for(server: &MockServer, key: &str) -> MemDirectory {
    let mut dir = MemDirectory::default();
    let base = format!("{}/api", server.uri());
    dir.install("kavita_1", "all", &[("APIURL", &base), ("APIKEY", key)]);
    dir
}

fn api_for(dir: MemDirectory) -> KavitaApi {
    let http = tracker_client().unwrap();
    KavitaApi::new(http.clone(), KavitaAuth::new(Arc::new(dir), http))
}

/// Authenticate answers with `tok` for the key `K1`.
async fn login_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/Plugin/authenticate"))
        .and(query_param("apiKey", KEY))
        .respond_with(ResponseTemplate::new(200).set_body_string(TOKEN_BODY))
        .mount(&server)
        .await;
    server
}

fn series_url(server: &MockServer) -> String {
    format!("{}/api/series/12", server.uri())
}

/// A GET that answers only with the given bearer token.
async fn get(server: &MockServer, token: &str, p: &str, status: u16, body: &str) {
    Mock::given(method("GET"))
        .and(path(p))
        .and(header("authorization", format!("Bearer {token}").as_str()))
        .respond_with(ResponseTemplate::new(status).set_body_string(body.to_owned()))
        .mount(server)
        .await;
}

/// The three reads of a series, all answered for the token `tok`.
async fn mount_reads(server: &MockServer, series: &str, volumes: &str, latest: (u16, &str)) {
    get(server, "tok", "/api/series/12", 200, series).await;
    get(server, "tok", "/api/Series/volumes", 200, volumes).await;
    get(
        server,
        "tok",
        "/api/Tachiyomi/latest-chapter",
        latest.0,
        latest.1,
    )
    .await;
}

fn track(url: &str) -> MangaTrack {
    MangaTrack {
        id: Some(1),
        manga_id: 1,
        tracker_id: TrackerId::KAVITA,
        remote_id: 0,
        library_id: None,
        title: "T".into(),
        last_chapter_read: 3.0,
        total_chapters: 0,
        status: READING,
        score: 8.0,
        remote_url: url.into(),
        start_date: 0,
        finish_date: 0,
        private: false,
    }
}

#[tokio::test]
async fn t_33a_057_e15_a_series_without_a_thumbnail_has_an_empty_cover() {
    let server = login_server().await;
    mount_reads(&server, SERIES_OK, VOLUMES_OK, (204, "")).await;
    let found = api_for(dir_for(&server, KEY))
        .get_track_search(&series_url(&server))
        .await
        .unwrap();
    assert_eq!(found.cover_url, "");
    assert_eq!(found.total_chapters, 10);
}

#[tokio::test]
async fn t_33a_057_e16_a_null_thumbnail_is_an_empty_cover_not_the_word_null() {
    let server = login_server().await;
    let series = r#"{"id":12,"name":"B","pages":10,"pagesRead":4,"format":3,"libraryId":1,"thumbnail_url":null}"#;
    mount_reads(&server, series, VOLUMES_OK, (204, "")).await;
    let found = api_for(dir_for(&server, KEY))
        .get_track_search(&series_url(&server))
        .await
        .unwrap();
    assert_eq!(found.cover_url, "");
}

#[tokio::test]
async fn t_33a_057_e17_an_empty_series_is_unread() {
    let server = login_server().await;
    let series = r#"{"id":12,"name":"B","pages":0,"pagesRead":0,"format":3,"libraryId":1}"#;
    mount_reads(&server, series, "[]", (204, "")).await;
    let found = api_for(dir_for(&server, KEY))
        .get_track_search(&series_url(&server))
        .await
        .unwrap();
    assert_eq!(found.status, UNREAD);
}

#[tokio::test]
async fn t_33a_057_e18_a_fully_read_series_is_completed() {
    let server = login_server().await;
    let series = r#"{"id":12,"name":"B","pages":10,"pagesRead":10,"format":3,"libraryId":1}"#;
    mount_reads(&server, series, "[]", (204, "")).await;
    let found = api_for(dir_for(&server, KEY))
        .get_track_search(&series_url(&server))
        .await
        .unwrap();
    assert_eq!(found.status, COMPLETED);
}

#[tokio::test]
async fn t_33a_057_e28_a_comma_decimal_latest_chapter_is_read() {
    let server = login_server().await;
    mount_reads(
        &server,
        SERIES_OK,
        VOLUMES_OK,
        (200, r#"{"number":"12,5"}"#),
    )
    .await;
    let found = api_for(dir_for(&server, KEY))
        .get_track_search(&series_url(&server))
        .await
        .unwrap();
    assert_eq!(found.last_chapter_read, 12.5);
}

#[tokio::test]
async fn t_33a_057_e29_e31_no_content_and_a_server_error_are_zero() {
    for (status, body) in [(204, ""), (500, "boom")] {
        let server = login_server().await;
        mount_reads(&server, SERIES_OK, VOLUMES_OK, (status, body)).await;
        let found = api_for(dir_for(&server, KEY))
            .get_track_search(&series_url(&server))
            .await
            .unwrap();
        assert_eq!(found.last_chapter_read, 0.0, "status {status}");
    }
}

#[tokio::test]
async fn t_33a_057_e30_a_negative_latest_chapter_is_clamped_to_zero() {
    let server = login_server().await;
    mount_reads(&server, SERIES_OK, VOLUMES_OK, (200, r#"{"number":"-1"}"#)).await;
    let found = api_for(dir_for(&server, KEY))
        .get_track_search(&series_url(&server))
        .await
        .unwrap();
    assert_eq!(found.last_chapter_read, 0.0);
}

#[tokio::test]
async fn t_33a_057_e32_e34_the_mark_request_uses_kotlin_text_and_keeps_the_server_status() {
    let server = login_server().await;
    Mock::given(method("POST"))
        .and(path("/api/Tachiyomi/mark-chapter-until-as-read"))
        .and(header("authorization", "Bearer tok"))
        .and(query_param("seriesId", "12"))
        .and(query_param("chapterNumber", "12.0"))
        .respond_with(ResponseTemplate::new(200).set_body_string("true"))
        .expect(1)
        .mount(&server)
        .await;
    let series = r#"{"id":12,"name":"B","pages":10,"pagesRead":10,"format":3,"libraryId":1}"#;
    mount_reads(&server, series, VOLUMES_OK, (200, r#"{"number":"12.0"}"#)).await;
    let mut local = track(&series_url(&server));
    local.last_chapter_read = 12.0;
    let updated = api_for(dir_for(&server, KEY))
        .update_progress(&local)
        .await
        .unwrap();
    // E-92: the local READING is replaced by the server's COMPLETED.
    assert_eq!(updated.status, COMPLETED);
    assert_eq!(updated.last_chapter_read, 12.0);
    assert_eq!(updated.total_chapters, 10);
}

#[tokio::test]
async fn t_33a_057_e33_a_fraction_is_sent_as_it_is() {
    let server = login_server().await;
    Mock::given(method("POST"))
        .and(path("/api/Tachiyomi/mark-chapter-until-as-read"))
        .and(query_param("chapterNumber", "0.5"))
        .respond_with(ResponseTemplate::new(200).set_body_string("true"))
        .expect(1)
        .mount(&server)
        .await;
    mount_reads(&server, SERIES_OK, VOLUMES_OK, (204, "")).await;
    let mut local = track(&series_url(&server));
    local.last_chapter_read = 0.5;
    api_for(dir_for(&server, KEY))
        .update_progress(&local)
        .await
        .unwrap();
}

#[tokio::test]
async fn t_33a_057_e09_a_401_on_a_read_logs_in_again_and_retries_once() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/Plugin/authenticate"))
        .respond_with(ResponseTemplate::new(200).set_body_string(TOKEN_BODY))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/Plugin/authenticate"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"token":"tok2"}"#))
        .mount(&server)
        .await;
    // The first token is refused. The second one reads the series.
    get(&server, "tok", "/api/series/12", 401, "").await;
    get(&server, "tok2", "/api/series/12", 200, SERIES_OK).await;
    get(&server, "tok2", "/api/Series/volumes", 200, VOLUMES_OK).await;
    get(&server, "tok2", "/api/Tachiyomi/latest-chapter", 204, "").await;
    let found = api_for(dir_for(&server, KEY))
        .get_track_search(&series_url(&server))
        .await
        .unwrap();
    assert_eq!(found.total_chapters, 10);
    assert_eq!(found.status, READING);
}

#[tokio::test]
async fn t_33a_057_e93_a_failed_series_read_is_an_error_not_a_none() {
    let server = login_server().await;
    get(&server, "tok", "/api/series/12", 500, "boom").await;
    let tracker = KavitaTracker::new(
        Arc::new(dir_for(&server, KEY)),
        Arc::new(MemFlags::default()),
    )
    .unwrap();
    let entry = EntryRef {
        id: 1,
        source_id: 5,
        url: series_url(&server),
        title: "T".into(),
    };
    assert!(
        tracker
            .as_enhanced_manga()
            .unwrap()
            .match_entry(&entry)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn t_33a_057_e91_a_refresh_keeps_the_local_score_and_takes_the_server_progress() {
    let server = login_server().await;
    mount_reads(&server, SERIES_OK, VOLUMES_OK, (200, r#"{"number":"4"}"#)).await;
    let tracker = KavitaTracker::new(
        Arc::new(dir_for(&server, KEY)),
        Arc::new(MemFlags::default()),
    )
    .unwrap();
    let local = track(&series_url(&server));
    let refreshed = tracker.as_manga().unwrap().refresh(&local).await.unwrap();
    assert_eq!(refreshed.score, 8.0);
    assert_eq!(refreshed.last_chapter_read, 4.0);
    assert_eq!(refreshed.total_chapters, 10);
}

#[tokio::test]
async fn t_33a_057_no_slot_means_no_request_at_all() {
    let server = MockServer::start().await;
    let api = api_for(MemDirectory::default());
    assert!(api.get_track_search(&series_url(&server)).await.is_err());
    assert!(server.received_requests().await.unwrap().is_empty());
}
