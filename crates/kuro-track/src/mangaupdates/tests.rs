//! Tracker-level tests of MangaUpdates against a local HTTP server (`wiremock`). Each test names the task it covers.

use std::sync::Arc;

use wiremock::matchers::{body_string, header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

use super::api::USER_AGENT;
use super::error::MuError;
use super::*;
use crate::fakes::MemSecrets;
use crate::model::{MangaTrack, TrackerId, copy_personal_from_manga};

fn tracker(server: &MockServer) -> (MangaUpdatesTracker, Arc<MemSecrets>) {
    let secrets = MemSecrets::new();
    let dyn_secrets: Arc<dyn SecretStore> = secrets.clone();
    let t = MangaUpdatesTracker::with_base_url(&server.uri(), dyn_secrets, EventBus::default())
        .unwrap();
    (t, secrets)
}

/// A tracker with a session (uid 42, token `T`).
fn signed_in(server: &MockServer) -> MangaUpdatesTracker {
    let (t, _) = tracker(server);
    t.save_credentials("42", "T").unwrap();
    t
}

async fn reply(server: &MockServer, verb: &str, at: &str, status: u16, body: &str) {
    Mock::given(method(verb))
        .and(path(at))
        .respond_with(ResponseTemplate::new(status).set_body_string(body.to_owned()))
        .mount(server)
        .await;
}

async fn sent(server: &MockServer) -> Vec<Request> {
    server.received_requests().await.unwrap_or_default()
}

fn track(remote_id: i64) -> MangaTrack {
    MangaTrack {
        id: None,
        manga_id: 1,
        tracker_id: TRACKER_ID,
        remote_id,
        library_id: None,
        title: "Example".into(),
        last_chapter_read: 0.0,
        total_chapters: 0,
        status: 0,
        score: 0.0,
        remote_url: String::new(),
        start_date: 0,
        finish_date: 0,
        private: false,
    }
}

fn offline() -> MangaUpdatesTracker {
    let secrets: Arc<dyn SecretStore> = MemSecrets::new();
    MangaUpdatesTracker::with_base_url("http://127.0.0.1:9", secrets, EventBus::default()).unwrap()
}

// ---- T-32a-001, T-32a-010, T-32a-003, T-32a-026: identity, lists and scores ----

#[test]
fn t_32a_001_the_tracker_id_and_identity_are_the_source_values() {
    let t = offline();
    assert_eq!(TRACKER_ID, TrackerId::MANGAUPDATES);
    assert_eq!(TRACKER_ID.0, 7);
    assert_eq!(t.id(), TrackerId::MANGAUPDATES);
    assert_eq!(t.name(), "MangaUpdates");
    assert_eq!(t.completion_status(), 2);
    assert_eq!(t.logo_color(), 0xFF92A0AD);
    assert!(t.as_manga().is_some() && t.as_deletable_manga().is_some());
    assert!(t.as_anime().is_none());
}

#[test]
fn t_32a_010_the_picker_order_and_the_reading_status_come_from_the_table() {
    let t = offline();
    assert_eq!(t.status_list(), vec![0, 2, 4, 3, 1]);
    assert_eq!(t.reading_status(), 0);
    assert_eq!(t.rereading_status(), -1);
    assert_eq!(t.status_label(0), Some("reading_list"));
    assert_eq!(t.status_label(9), None);
}

#[test]
fn t_32a_003_the_score_list_is_built_once_and_lent_by_reference() {
    let t = offline();
    assert_eq!(t.score_list().len(), 92);
    assert!(std::ptr::eq(t.score_list().as_ptr(), t.scores.as_ptr()));
    assert_eq!(t.index_to_score(91), 10.0);
    assert_eq!(
        t.index_to_score(92),
        0.0,
        "the trait cannot report an error"
    );
}

#[test]
fn t_32a_026_the_display_text_is_a_dash_for_zero() {
    let t = offline();
    assert_eq!(t.display_score(&track(1)), "-");
    let mut rated = track(1);
    rated.score = 7.5;
    assert_eq!(t.display_score(&rated), "7.5");
}

// ---- T-32a-002, T-32a-004: credentials and track model ----

#[test]
fn t_32a_002_logged_in_needs_both_credentials_and_logout_clears_the_expired_flag() {
    let t = offline();
    let state = t.login_state();
    assert!(!*state.borrow());
    t.save_credentials("42", "").unwrap();
    assert!(!t.is_logged_in());
    t.save_credentials("42", "T").unwrap();
    assert!(t.is_logged_in());
    assert!(*state.borrow(), "login state is true after a save");
    t.base.mark_expired().unwrap();
    assert!(t.auth_expired());
    t.save_credentials("42", "T").unwrap();
    assert!(!t.auth_expired(), "a save clears the expired flag");
    t.logout().unwrap();
    assert!(!t.is_logged_in());
    assert!(!*state.borrow(), "login state is false after logout");
    assert!(!t.auth_expired());
}

#[test]
fn t_32a_004_personal_fields_copy_and_the_remote_fields_stay() {
    let mut dst = track(101);
    dst.title = "Mine".into();
    dst.total_chapters = 5;
    dst.remote_url = "https://x/101".into();
    let mut src = track(101);
    src.last_chapter_read = 3.0;
    src.score = 7.0;
    src.status = 4;
    src.title = "Remote".into();
    copy_personal_from_manga(&mut dst, &src, false);
    assert_eq!(dst.last_chapter_read, 3.0);
    assert_eq!(dst.score, 7.0);
    assert_eq!(dst.status, 4);
    assert_eq!(dst.title, "Mine");
    assert_eq!(dst.total_chapters, 5);
    assert_eq!(dst.remote_url, "https://x/101");
}

#[test]
fn t_32a_004_a_new_mangaupdates_track_has_no_remote_url_and_round_trips_the_row() {
    let mut t = track(101);
    assert_eq!(t.remote_url, "", "never null");
    assert!(!t.private);
    t.id = Some(5);
    t.score = 9.5;
    let row = t.to_db(true).unwrap();
    assert_eq!(MangaTrack::from_db(&row), t);
}

// ---- T-32a-011, T-32a-013, T-32a-014: HTTP client, login and session ----

#[tokio::test]
async fn t_32a_013_login_stores_the_uid_and_the_token_never_the_typed_password() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v1/account/login"))
        .and(body_string(r#"{"username":"ann","password":"pw"}"#))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"context":{"session_token":"T","uid":42}}"#),
        )
        .mount(&server)
        .await;
    let (t, secrets) = tracker(&server);
    t.login("ann", "pw").await.unwrap();
    assert_eq!(t.username(), "42");
    assert_eq!(t.password(), "T");
    assert!(t.is_logged_in());
    assert!(!secrets.all_values().iter().any(|v| v == "pw" || v == "ann"));
}

#[tokio::test]
async fn t_32a_013_a_refused_login_stores_nothing_and_reports_the_code() {
    let server = MockServer::start().await;
    reply(&server, "PUT", "/v1/account/login", 401, "no").await;
    let (t, _) = tracker(&server);
    let err = t.login("ann", "bad").await.unwrap_err();
    assert_eq!(err.to_string(), "HTTP 401");
    assert!(!t.is_logged_in());
    assert_eq!(t.username(), "");
}

#[tokio::test]
async fn t_32a_013_a_login_reply_without_context_is_a_decode_error() {
    let server = MockServer::start().await;
    reply(&server, "PUT", "/v1/account/login", 200, "{}").await;
    let (t, _) = tracker(&server);
    let err = t.api.authenticate("ann", "pw").await.unwrap_err();
    assert!(matches!(err, MuError::Decode(_)));
}

#[tokio::test]
async fn t_32a_014_logout_clears_the_token_so_the_next_call_fails_without_a_request() {
    let server = MockServer::start().await;
    reply(
        &server,
        "GET",
        "/v1/lists/series/101",
        200,
        r#"{"list_id":0}"#,
    )
    .await;
    let t = signed_in(&server);
    t.logout().unwrap();
    let err = t.api.list_item(101).await.unwrap_err();
    assert_eq!(err, MuError::NotAuthenticated);
    assert!(sent(&server).await.is_empty(), "no request was made");
}

#[tokio::test]
async fn t_32a_014_a_restored_session_sends_the_stored_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/lists/series/101"))
        .and(header("authorization", "Bearer T"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"list_id":0}"#))
        .mount(&server)
        .await;
    let (first, secrets) = tracker(&server);
    first.save_credentials("42", "T").unwrap();
    let dyn_secrets: Arc<dyn SecretStore> = secrets.clone();
    let restored =
        MangaUpdatesTracker::with_base_url(&server.uri(), dyn_secrets, EventBus::default())
            .unwrap();
    assert!(restored.api.list_item(101).await.is_ok());
}

#[tokio::test]
async fn t_32a_011_authenticated_calls_send_the_bearer_and_the_app_agent_and_search_sends_no_token()
{
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/lists/series/101"))
        .and(header("authorization", "Bearer T"))
        .and(header("user-agent", USER_AGENT))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"list_id":0}"#))
        .mount(&server)
        .await;
    reply(
        &server,
        "POST",
        "/v1/series/search",
        200,
        r#"{"results":[]}"#,
    )
    .await;
    let t = signed_in(&server);
    t.api.list_item(101).await.unwrap();
    t.search("abc").await.unwrap();
    let requests = sent(&server).await;
    let search = requests
        .iter()
        .find(|r| r.url.path() == "/v1/series/search")
        .unwrap();
    assert!(!search.headers.contains_key("authorization"));
    assert!(USER_AGENT.starts_with("Kurobayomi v"));
}

// ---- T-32a-015 to T-32a-018: bind, add, rating tri-state ----

#[tokio::test]
async fn t_32a_015_bind_copies_the_list_item_and_the_remote_rating() {
    let server = MockServer::start().await;
    reply(
        &server,
        "GET",
        "/v1/lists/series/101",
        200,
        r#"{"list_id":2,"status":{"chapter":12}}"#,
    )
    .await;
    reply(
        &server,
        "GET",
        "/v1/series/101/rating",
        200,
        r#"{"rating":7.5}"#,
    )
    .await;
    let t = signed_in(&server);
    let mut row = track(101);
    t.bind(&mut row, false).await.unwrap();
    assert_eq!(row.status, 2);
    assert_eq!(row.last_chapter_read, 12.0);
    assert_eq!(row.score, 7.5);
}

#[tokio::test]
async fn t_32a_015_a_list_404_adds_to_the_wish_list_when_not_read() {
    let server = MockServer::start().await;
    reply(&server, "GET", "/v1/lists/series/101", 404, "").await;
    Mock::given(method("POST"))
        .and(path("/v1/lists/series"))
        .and(body_string(r#"[{"series":{"id":101},"list_id":1}]"#))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let t = signed_in(&server);
    let mut row = track(101);
    row.last_chapter_read = 4.0;
    t.bind(&mut row, false).await.unwrap();
    assert_eq!(row.status, 1);
    assert_eq!(
        row.last_chapter_read, 4.0,
        "not read, so the chapter is unchanged"
    );
}

#[tokio::test]
async fn t_32a_016_a_read_series_adds_to_reading_and_sets_chapter_one_on_200() {
    let server = MockServer::start().await;
    reply(&server, "GET", "/v1/lists/series/101", 404, "").await;
    Mock::given(method("POST"))
        .and(path("/v1/lists/series"))
        .and(body_string(r#"[{"series":{"id":101},"list_id":0}]"#))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let t = signed_in(&server);
    let mut row = track(101);
    t.bind(&mut row, true).await.unwrap();
    assert_eq!(row.status, 0);
    assert_eq!(row.last_chapter_read, 1.0);
}

#[tokio::test]
async fn t_32a_016_a_201_add_does_not_write_the_status() {
    let server = MockServer::start().await;
    reply(&server, "POST", "/v1/lists/series", 201, "").await;
    let t = signed_in(&server);
    let mut row = track(101);
    row.status = 9;
    t.add(&mut row, false).await.unwrap();
    assert_eq!(row.status, 9, "status is written only on 200");
}

#[tokio::test]
async fn t_32a_015_a_list_failure_other_than_404_stops_the_bind_with_no_add() {
    for code in [500u16, 401] {
        let server = MockServer::start().await;
        reply(&server, "GET", "/v1/lists/series/101", code, "").await;
        let t = signed_in(&server);
        let mut row = track(101);
        assert!(t.bind(&mut row, false).await.is_err(), "status {code}");
        let requests = sent(&server).await;
        assert_eq!(requests.len(), 1, "only the lookup was sent for {code}");
    }
}

#[tokio::test]
async fn t_32a_018_a_failed_rating_fetch_keeps_the_previous_score() {
    let server = MockServer::start().await;
    reply(
        &server,
        "GET",
        "/v1/lists/series/101",
        200,
        r#"{"list_id":0}"#,
    )
    .await;
    reply(&server, "GET", "/v1/series/101/rating", 500, "").await;
    let t = signed_in(&server);
    let mut row = track(101);
    row.score = 4.0;
    t.bind(&mut row, false).await.unwrap();
    assert_eq!(row.score, 4.0, "an unknown rating is not a zero");
}

#[tokio::test]
async fn t_32a_018_a_known_absent_rating_gives_zero() {
    let server = MockServer::start().await;
    reply(
        &server,
        "GET",
        "/v1/lists/series/101",
        200,
        r#"{"list_id":0}"#,
    )
    .await;
    reply(
        &server,
        "GET",
        "/v1/series/101/rating",
        200,
        r#"{"rating":null}"#,
    )
    .await;
    let t = signed_in(&server);
    let mut row = track(101);
    row.score = 4.0;
    t.bind(&mut row, false).await.unwrap();
    assert_eq!(row.score, 0.0);
    assert_eq!(remembered_in(&t.remote_ratings, 101), Some(None));
}

// ---- T-32a-017, T-32a-019, T-32a-020, T-32a-022: update, delete, refresh ----

#[tokio::test]
async fn t_32a_017_the_list_update_body_keeps_key_order_and_truncates_the_chapter() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/lists/series/update"))
        .and(body_string(
            r#"[{"series":{"id":101},"list_id":0,"status":{"chapter":12}}]"#,
        ))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let t = signed_in(&server);
    let mut row = track(101);
    row.last_chapter_read = 12.9;
    row.score = -1.0;
    t.update(&row, false).await.unwrap();
    assert_eq!(
        sent(&server).await.len(),
        1,
        "a negative score sends no rating request"
    );
}

#[tokio::test]
async fn t_32a_017_a_score_is_put_as_the_rating_after_the_list_update() {
    let server = MockServer::start().await;
    reply(&server, "POST", "/v1/lists/series/update", 200, "").await;
    Mock::given(method("PUT"))
        .and(path("/v1/series/101/rating"))
        .and(body_string(r#"{"rating":7.5}"#))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let t = signed_in(&server);
    let mut row = track(101);
    row.score = 7.5;
    t.update(&row, false).await.unwrap();
    assert_eq!(sent(&server).await.len(), 2);
}

#[tokio::test]
async fn t_32a_017_a_zero_score_deletes_only_a_rating_the_server_is_known_to_hold() {
    let server = MockServer::start().await;
    reply(&server, "POST", "/v1/lists/series/update", 200, "").await;
    reply(&server, "DELETE", "/v1/series/101/rating", 200, "").await;
    let t = signed_in(&server);
    let mut row = track(101);
    row.score = 0.0;

    t.remember(101, Some(7.0));
    t.update(&row, false).await.unwrap();
    let deletes = sent(&server)
        .await
        .iter()
        .filter(|r| r.method.as_str() == "DELETE")
        .count();
    assert_eq!(deletes, 1, "known present: DELETE");

    t.remember(101, None);
    t.update(&row, false).await.unwrap();
    let deletes = sent(&server)
        .await
        .iter()
        .filter(|r| r.method.as_str() == "DELETE")
        .count();
    assert_eq!(deletes, 1, "known absent: no DELETE");
}

#[tokio::test]
async fn t_32a_017_a_failed_rating_lookup_sends_no_delete() {
    let server = MockServer::start().await;
    reply(&server, "POST", "/v1/lists/series/update", 200, "").await;
    reply(&server, "GET", "/v1/series/101/rating", 500, "").await;
    let t = signed_in(&server);
    t.update(&track(101), false).await.unwrap();
    let requests = sent(&server).await;
    assert!(requests.iter().all(|r| r.method.as_str() != "DELETE"));
}

#[tokio::test]
async fn t_32a_017_a_failed_list_update_stops_before_the_rating_step() {
    let server = MockServer::start().await;
    reply(&server, "POST", "/v1/lists/series/update", 500, "").await;
    let t = signed_in(&server);
    let mut row = track(101);
    row.score = 7.5;
    assert!(t.update(&row, false).await.is_err());
    assert_eq!(sent(&server).await.len(), 1);
}

#[tokio::test]
async fn t_32a_022_update_moves_a_non_complete_series_to_reading_when_a_chapter_was_read() {
    let server = MockServer::start().await;
    reply(&server, "POST", "/v1/lists/series/update", 200, "").await;
    let t = signed_in(&server);
    let mut complete = track(101);
    complete.status = 2;
    complete.score = -1.0;
    assert_eq!(t.update(&complete, true).await.unwrap().status, 2);
    let mut hold = track(101);
    hold.status = 4;
    hold.score = -1.0;
    assert_eq!(t.update(&hold, true).await.unwrap().status, 0);
    let mut wish = track(101);
    wish.status = 1;
    wish.score = -1.0;
    assert_eq!(t.update(&wish, false).await.unwrap().status, 1);
}

#[tokio::test]
async fn t_32a_019_delete_sends_the_one_element_array_and_changes_no_local_state() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/lists/series/delete"))
        .and(body_string("[101]"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let t = signed_in(&server);
    let row = track(101);
    DeletableMangaTracker::delete(&t, &row).await.unwrap();
    assert_eq!(row, track(101));
}

#[tokio::test]
async fn t_32a_020_refresh_copies_status_chapter_and_score_and_keeps_the_identity() {
    let server = MockServer::start().await;
    reply(
        &server,
        "GET",
        "/v1/lists/series/101",
        200,
        r#"{"list_id":4,"status":{"chapter":3}}"#,
    )
    .await;
    reply(
        &server,
        "GET",
        "/v1/series/101/rating",
        200,
        r#"{"rating":6.0}"#,
    )
    .await;
    let t = signed_in(&server);
    let mut row = track(101);
    row.remote_url = "https://x".into();
    let fresh = t.refresh(&row).await.unwrap();
    assert_eq!(fresh.status, 4);
    assert_eq!(fresh.last_chapter_read, 3.0);
    assert_eq!(fresh.score, 6.0);
    assert_eq!(fresh.title, "Example");
    assert_eq!(fresh.remote_id, 101);
    assert_eq!(fresh.remote_url, "https://x");
}

#[tokio::test]
async fn t_32a_020_refresh_keeps_the_score_when_the_rating_fails_and_fails_when_the_list_fails() {
    let server = MockServer::start().await;
    reply(
        &server,
        "GET",
        "/v1/lists/series/101",
        200,
        r#"{"list_id":0}"#,
    )
    .await;
    reply(&server, "GET", "/v1/series/101/rating", 500, "").await;
    let t = signed_in(&server);
    let mut row = track(101);
    row.score = 3.0;
    assert_eq!(t.refresh(&row).await.unwrap().score, 3.0);

    let failing = MockServer::start().await;
    reply(&failing, "GET", "/v1/lists/series/101", 500, "").await;
    let t2 = signed_in(&failing);
    assert!(t2.refresh(&row).await.is_err());
}

// ---- T-32a-021, T-32a-023: search ----

#[tokio::test]
async fn t_32a_021_search_sends_the_exact_body_without_a_token_and_maps_null_fields_to_empty() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/series/search"))
        .and(body_string(r#"{"search":"a \"b\" 漫","filter_types":["drama cd","novel"]}"#))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"results":[{"record":{"series_id":5,"title":"X &amp; Y","type":null,"year":null}}]}"#,
        ))
        .mount(&server)
        .await;
    let (t, _) = tracker(&server);
    let rows = t.search("a \"b\" 漫").await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "X & Y");
    assert_eq!(rows[0].publishing_type, "");
    assert_eq!(rows[0].start_date, "");
    assert_eq!(rows[0].total_chapters, 0);
    assert_eq!(rows[0].score, None);
}

#[tokio::test]
async fn t_32a_021_an_empty_search_is_an_empty_list() {
    let server = MockServer::start().await;
    reply(
        &server,
        "POST",
        "/v1/series/search",
        200,
        r#"{"results":[]}"#,
    )
    .await;
    let (t, _) = tracker(&server);
    assert!(t.search("none").await.unwrap().is_empty());
}

// ---- T-32a-060, T-32a-066: refusal handling and write safety ----

#[tokio::test]
async fn t_32a_060_a_refused_call_marks_the_login_expired() {
    let server = MockServer::start().await;
    reply(&server, "GET", "/v1/lists/series/101", 401, "no").await;
    let t = signed_in(&server);
    assert!(t.refresh(&track(101)).await.is_err());
    assert!(t.auth_expired());
    assert!(!t.is_logged_in());
}

#[tokio::test]
async fn t_32a_060_an_error_snippet_is_redacted() {
    let server = MockServer::start().await;
    reply(
        &server,
        "POST",
        "/v1/series/search",
        500,
        r#"{"message":"bad","session_token":"SECRET-VALUE"}"#,
    )
    .await;
    let api = api::MuApi::new(&server.uri(), Arc::new(auth::MuAuth::new(None))).unwrap();
    match api.search("x").await {
        Err(MuError::Http {
            status,
            body_snippet,
        }) => {
            assert_eq!(status, 500);
            assert!(!body_snippet.contains("SECRET-VALUE"), "{body_snippet}");
        }
        other => panic!("expected an HTTP error, got {other:?}"),
    }
}

#[tokio::test]
async fn t_32a_066_a_dropped_caller_does_not_stop_the_rating_step() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/lists/series/update"))
        .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_millis(40)))
        .mount(&server)
        .await;
    reply(&server, "PUT", "/v1/series/101/rating", 200, "").await;
    let t = signed_in(&server);
    let mut row = track(101);
    row.score = 7.5;
    let caller =
        tokio::time::timeout(std::time::Duration::from_millis(10), t.update(&row, false)).await;
    assert!(caller.is_err(), "the caller should have been dropped");
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    let paths: Vec<String> = sent(&server)
        .await
        .iter()
        .map(|r| r.url.path().to_owned())
        .collect();
    assert!(
        paths.contains(&"/v1/series/101/rating".to_owned()),
        "{paths:?}"
    );
}

// ---- T-32a-069: live checks (O-1), run by hand with a throwaway account ----

fn live_tracker(token: &str) -> MangaUpdatesTracker {
    let secrets: Arc<dyn SecretStore> = MemSecrets::new();
    let t = MangaUpdatesTracker::new(secrets, EventBus::default()).unwrap();
    t.save_credentials("0", token).unwrap();
    t
}

/// TST-74 (O-1): a series the account does not list gives 404 on the list item, so bind adds it.
/// Run with: `KUROBAYOMI_MU_SESSION=<token> cargo test -p kuro-track -- --ignored live_t_32a_074`.
#[tokio::test]
#[ignore = "live: needs KUROBAYOMI_MU_SESSION from a throwaway MangaUpdates account"]
async fn live_t_32a_074_an_unlisted_series_is_404_on_the_list_item() {
    let token = std::env::var("KUROBAYOMI_MU_SESSION").expect("set KUROBAYOMI_MU_SESSION");
    let t = live_tracker(&token);
    let err = t.api.list_item(1).await.unwrap_err();
    assert!(matches!(err, MuError::Http { status: 404, .. }), "{err:?}");
}

/// TST-75 (O-1): a series with no rating from the account gives a known absent rating, not a failure.
#[tokio::test]
#[ignore = "live: needs KUROBAYOMI_MU_SESSION from a throwaway MangaUpdates account"]
async fn live_t_32a_075_a_series_without_a_rating_is_known_absent() {
    let token = std::env::var("KUROBAYOMI_MU_SESSION").expect("set KUROBAYOMI_MU_SESSION");
    let t = live_tracker(&token);
    assert_eq!(t.api.rating(1).await, api::RatingFetch::Known(None));
}
