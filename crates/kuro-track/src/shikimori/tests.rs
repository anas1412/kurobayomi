//! Shikimori tests: identity and rules (pure), and wiremock tests for every endpoint of `p1-data-and-config.md` 7.1.

use std::sync::Arc;

use kuro_platform::secrets::SecretStore;
use serde_json::json;
use wiremock::matchers::{body_string, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::api::{Endpoints, OAuthClient, USER_AGENT_VALUE};
use super::dto::SmOAuth;
use super::*;
use crate::fakes::MemSecrets;
use crate::model::{AnimeTrack, MangaTrack};

const REDIRECT: &str = "kurobayomi://shikimori-auth";
const TOKEN_BODY: &str = "grant_type=authorization_code&client_id=cid&client_secret=sec&code=abc&redirect_uri=kurobayomi%3A%2F%2Fshikimori-auth";

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn token(access: &str, created: i64, refresh: Option<&str>) -> SmOAuth {
    SmOAuth {
        access_token: access.into(),
        token_type: "Bearer".into(),
        created_at: created,
        expires_in: 86_400,
        refresh_token: refresh.map(str::to_owned),
    }
}

async fn tracker_for(server: &MockServer, secrets: Arc<dyn SecretStore>) -> ShikimoriTracker {
    ShikimoriTracker::new(
        Endpoints {
            base: server.uri(),
        },
        Some(OAuthClient {
            client_id: "cid".into(),
            client_secret: "sec".into(),
        }),
        REDIRECT,
        secrets,
    )
    .unwrap()
}

/// A tracker logged in as user 1234 with a fresh token `tok`.
async fn logged_in(server: &MockServer) -> (ShikimoriTracker, Arc<dyn SecretStore>) {
    let secrets: Arc<dyn SecretStore> = MemSecrets::new();
    let t = tracker_for(server, secrets.clone()).await;
    let fresh = token("tok", now(), Some("r"));
    t.api().base().save_oauth("1234", "tok", &fresh).unwrap();
    t.api().tokens().login(fresh);
    (t, secrets)
}

fn manga(status: i64, read: f64, total: i64) -> MangaTrack {
    MangaTrack {
        id: None,
        manga_id: 1,
        tracker_id: TrackerId::SHIKIMORI,
        remote_id: 5,
        library_id: None,
        title: "T".into(),
        last_chapter_read: read,
        total_chapters: total,
        status,
        score: 0.0,
        remote_url: String::new(),
        start_date: 0,
        finish_date: 0,
        private: false,
    }
}

fn anime(status: i64, seen: f64, total: i64) -> AnimeTrack {
    AnimeTrack {
        id: None,
        anime_id: 1,
        tracker_id: TrackerId::SHIKIMORI,
        remote_id: 5,
        library_id: None,
        title: "T".into(),
        last_episode_seen: seen,
        total_episodes: total,
        status,
        score: 0.0,
        remote_url: String::new(),
        start_date: 0,
        finish_date: 0,
        private: false,
    }
}

fn catalogue(chapters: serde_json::Value) -> serde_json::Value {
    json!({"id": 5, "name": "Name", "chapters": chapters, "episodes": 12,
        "image": {"preview": "/images/p.jpg"}, "score": 7.5, "url": "/mangas/5",
        "status": "ongoing", "kind": "manga", "aired_on": "2020-01-02"})
}

// ---- T-31a-001, T-31a-002, T-31a-003, T-31a-012: identity, tables, rules ----

#[test]
fn t_31a_001_identity_and_status_tables() {
    let secrets: Arc<dyn SecretStore> = MemSecrets::new();
    let t = ShikimoriTracker::new(Endpoints::production(), None, REDIRECT, secrets).unwrap();
    assert_eq!(t.id(), TrackerId::SHIKIMORI);
    assert_eq!(t.id().0, 4);
    assert_eq!(t.name(), "Shikimori");
    assert!(!t.supports_private_tracking());
    assert!(!t.supports_reading_dates());
    assert_eq!(t.completion_status(), 2);
    assert_eq!(t.logo_color(), 0xFF28_2828);
    let m = t.as_manga().unwrap();
    assert_eq!((m.reading_status(), m.rereading_status()), (1, 6));
    assert_eq!(m.status_list(), vec![1, 2, 3, 4, 5, 6]);
    let a = t.as_anime().unwrap();
    assert_eq!((a.watching_status(), a.rewatching_status()), (1, 6));
    assert_eq!(a.status_list(), vec![1, 2, 3, 4, 5, 6]);
}

#[test]
fn t_31a_003_scores_are_zero_to_ten_and_display_truncates() {
    let secrets: Arc<dyn SecretStore> = MemSecrets::new();
    let t = ShikimoriTracker::new(Endpoints::production(), None, REDIRECT, secrets).unwrap();
    assert_eq!(t.score_list().len(), 11);
    assert_eq!(t.score_list()[10], "10");
    assert_eq!(t.as_manga().unwrap().index_to_score(3), 3.0);
    let mut m = manga(1, 0.0, 0);
    m.score = 7.9;
    assert_eq!(t.as_manga().unwrap().display_score(&m), "7");
}

#[test]
fn t_31a_012_read_status_transitions() {
    use ShikiStatus::*;
    assert_eq!(next_status_after_read(Completed, true, 5.0, 5), Completed);
    assert_eq!(next_status_after_read(Reading, false, 5.0, 5), Reading);
    assert_eq!(next_status_after_read(OnHold, true, 3.0, 12), Reading);
    assert_eq!(next_status_after_read(PlanToRead, true, 3.5, 12), Reading);
    assert_eq!(next_status_after_read(Rereading, true, 3.0, 12), Rereading);
    // Reaching the total completes even a rereading entry (the total check runs first).
    assert_eq!(next_status_after_read(Rereading, true, 12.9, 12), Completed);
    // A zero total never completes.
    assert_eq!(next_status_after_read(Reading, true, 12.0, 0), Reading);
}

// ---- T-31a-006: login by code ----

#[test]
fn t_31a_006_auth_url_has_the_source_parameter_order_and_no_scope() {
    let secrets: Arc<dyn SecretStore> = MemSecrets::new();
    let t = ShikimoriTracker::new(
        Endpoints::production(),
        Some(OAuthClient {
            client_id: "cid".into(),
            client_secret: "sec".into(),
        }),
        REDIRECT,
        secrets,
    )
    .unwrap();
    assert_eq!(
        t.auth_url().unwrap(),
        "https://shikimori.one/oauth/authorize?client_id=cid&redirect_uri=kurobayomi%3A%2F%2Fshikimori-auth&response_type=code"
    );
}

#[test]
fn t_31a_006_without_a_client_config_login_is_refused() {
    let secrets: Arc<dyn SecretStore> = MemSecrets::new();
    let t = ShikimoriTracker::new(Endpoints::production(), None, REDIRECT, secrets).unwrap();
    assert_eq!(t.auth_url(), Err(ShikimoriError::NotConfigured));
}

#[tokio::test]
async fn t_31a_006_login_stores_the_user_id_and_the_token_after_whoami() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string(TOKEN_BODY))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"access_token": "new", "token_type": "Bearer", "created_at": now(), "expires_in": 86400, "refresh_token": "rr"}),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/users/whoami"))
        .and(header("Authorization", "Bearer new"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 1234})))
        .expect(1)
        .mount(&server)
        .await;
    let secrets: Arc<dyn SecretStore> = MemSecrets::new();
    let t = tracker_for(&server, secrets.clone()).await;

    t.login("", "abc").await.unwrap();

    assert!(t.is_logged_in());
    assert_eq!(t.username(), "1234");
    assert_eq!(t.password(), "new");
    assert_eq!(t.api().base().load_token::<SmOAuth>().unwrap().unwrap().access_token, "new");
}

#[tokio::test]
async fn t_31a_006_a_refused_code_stores_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(400))
        .mount(&server)
        .await;
    let secrets: Arc<dyn SecretStore> = MemSecrets::new();
    let t = tracker_for(&server, secrets).await;

    assert!(t.login("", "abc").await.is_err());
    assert!(!t.is_logged_in());
    assert!(t.api().base().load_token::<SmOAuth>().unwrap().is_none());
}

#[tokio::test]
async fn t_31a_006_a_failed_whoami_keeps_an_existing_login() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"access_token": "second", "token_type": "Bearer", "created_at": now(), "expires_in": 86400}),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/users/whoami"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;

    assert!(t.login("", "abc").await.is_err());

    assert!(t.is_logged_in(), "RC-06: a failed second login does not log out");
    assert_eq!(t.username(), "1234");
    assert_eq!(t.password(), "tok");
}

#[tokio::test]
async fn t_31a_006_logout_clears_credentials_and_the_token() {
    let server = MockServer::start().await;
    let (t, _) = logged_in(&server).await;
    t.logout().unwrap();
    assert!(!t.is_logged_in());
    assert!(t.api().base().load_token::<SmOAuth>().unwrap().is_none());
    assert!(!t.api().tokens().has_token());
}

// ---- T-31a-005, T-31a-008: refresh and request headers ----

#[tokio::test]
async fn t_31a_005_ten_concurrent_callers_cause_one_refresh() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string(
            "grant_type=refresh_token&client_id=cid&client_secret=sec&refresh_token=r",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"access_token": "fresh", "token_type": "Bearer", "created_at": now(), "expires_in": 86400, "refresh_token": "r2"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;
    // Make the stored token expired.
    let old = token("tok", 0, Some("r"));
    t.api().tokens().login(old);

    let calls = (0..10).map(|_| t.api().bearer());
    let results = futures::future::join_all(calls).await;

    assert!(results.iter().all(|r| r.as_deref() == Ok("fresh")));
    assert_eq!(t.api().base().load_token::<SmOAuth>().unwrap().unwrap().refresh_token.as_deref(), Some("r2"));
}

#[tokio::test]
async fn t_31a_005_a_refusal_flags_the_login_and_sends_no_old_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(400))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;
    t.api().tokens().login(token("old", 0, Some("r")));

    let err = t.api().bearer().await.unwrap_err();

    assert_eq!(err, ShikimoriError::AuthExpired);
    assert!(t.auth_expired());
}

#[tokio::test]
async fn t_31a_005_a_server_error_on_refresh_keeps_the_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;
    t.api().tokens().login(token("old", 0, Some("r")));

    assert_eq!(t.api().bearer().await, Err(ShikimoriError::Http(500)));
    assert!(t.api().tokens().has_token());
    assert!(!t.auth_expired());
}

#[tokio::test]
async fn t_31a_005_expired_without_refresh_token_is_auth_expired_with_no_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;
    t.api().tokens().login(token("old", 0, None));

    assert_eq!(t.api().bearer().await, Err(ShikimoriError::AuthExpired));
}

#[tokio::test]
async fn t_31a_008_logged_out_calls_send_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let secrets: Arc<dyn SecretStore> = MemSecrets::new();
    let t = tracker_for(&server, secrets).await;

    let result = t.as_manga().unwrap().search("naruto").await;

    assert!(matches!(result, Err(TrackError::Key(_))));
    assert_eq!(
        ShikimoriError::NotAuthenticated.to_string(),
        "Not authenticated with Shikimori"
    );
}

#[tokio::test]
async fn t_31a_008_authed_calls_send_bearer_user_agent_and_no_cache() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/mangas"))
        .and(header("Authorization", "Bearer tok"))
        .and(header("User-Agent", USER_AGENT_VALUE))
        .and(header("Cache-Control", "no-cache"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;

    assert!(t.as_manga().unwrap().search("x").await.unwrap().is_empty());
}

// ---- T-31a-009, T-31a-010: search and lookup ----

#[tokio::test]
async fn t_31a_009_search_query_order_and_mapping() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/mangas"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([catalogue(json!(null))])))
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;

    let found = t.as_manga().unwrap().search("naruto").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests[0].url.query(), Some("order=popularity&search=naruto&limit=20"));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].remote_id, 5);
    assert_eq!(found[0].total_chapters, 0);
    assert_eq!(found[0].tracking_url, format!("{}/mangas/5", server.uri()));
    assert_eq!(found[0].cover_url, format!("{}/images/p.jpg", server.uri()));
    assert_eq!(found[0].start_date, "2020-01-02");
}

#[tokio::test]
async fn t_31a_010_two_rows_are_too_many_entries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/mangas/5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(catalogue(json!(20))))
        .mount(&server)
        .await;
    let row = json!({"id": 9, "chapters": 1, "episodes": 0, "score": 0, "status": "watching"});
    Mock::given(method("GET"))
        .and(path("/api/v2/user_rates"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([row.clone(), row])))
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;

    assert_eq!(
        t.api().find_library(Kind::Manga, 5, "1234").await,
        Err(ShikimoriError::TooManyEntries)
    );
}

#[tokio::test]
async fn t_31a_010_a_missing_catalogue_is_http_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/mangas/5"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;

    assert_eq!(
        t.api().find_library(Kind::Manga, 5, "1234").await,
        Err(ShikimoriError::Http(404))
    );
}

#[tokio::test]
async fn t_31a_010_refresh_sets_library_id_and_keeps_the_catalogue_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/mangas/5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(catalogue(json!(20))))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/user_rates"))
        .and(header("Cache-Control", "no-cache"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": 9, "chapters": 3.0, "episodes": 0.0, "score": 8, "status": "completed"}
        ])))
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;

    let got = t.as_manga().unwrap().refresh(&manga(1, 0.0, 0)).await.unwrap();

    assert_eq!(got.library_id, Some(9));
    assert_eq!(got.remote_id, 5);
    assert_eq!(got.last_chapter_read, 3.0);
    assert_eq!(got.score, 8.0);
    assert_eq!(got.status, 2);
    assert_eq!(got.total_chapters, 20);
}

#[tokio::test]
async fn t_31a_010_refresh_without_a_row_is_could_not_find_manga() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/mangas/5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(catalogue(json!(20))))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/user_rates"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;

    let err = t.as_manga().unwrap().refresh(&manga(1, 0.0, 0)).await.unwrap_err();

    assert_eq!(err.to_string(), "Could not find manga");
}

// ---- T-31a-011: the upsert body ----

#[tokio::test]
async fn t_31a_011_manga_upsert_body_is_exact_and_the_score_is_clamped() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/user_rates"))
        .and(header("Content-Type", "application/json; charset=utf-8"))
        .and(body_string(
            r#"{"user_rate":{"user_id":"1234","target_id":5,"target_type":"Manga","chapters":3,"score":10,"status":"watching"}}"#,
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 77})))
        .expect(1)
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;
    let mut m = manga(1, 3.9, 0);
    m.score = 12.0;

    let got = t.as_manga().unwrap().update(&m, false).await.unwrap();

    assert_eq!(got.library_id, Some(77));
    assert_eq!(got.status, 1);
}

#[tokio::test]
async fn t_31a_011_anime_upsert_uses_episodes_and_a_negative_score_is_zero() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/user_rates"))
        .and(body_string(
            r#"{"user_rate":{"user_id":"1234","target_id":5,"target_type":"Anime","episodes":4,"score":0,"status":"planned"}}"#,
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 78})))
        .expect(1)
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;
    let mut a = anime(5, 4.0, 12);
    a.score = -2.0;

    let got = t.as_anime().unwrap().update(&a, false).await.unwrap();

    assert_eq!(got.library_id, Some(78));
    assert_eq!(got.status, 5, "no read event, so the status is kept");
}

#[tokio::test]
async fn t_31a_011_an_unknown_internal_status_is_an_error_and_sends_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;

    assert!(t.as_manga().unwrap().update(&manga(9, 0.0, 0), false).await.is_err());
}

// ---- T-31a-012, T-31a-013: update on read, bind ----

#[tokio::test]
async fn t_31a_012_completing_the_last_chapter_sends_completed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string(
            r#"{"user_rate":{"user_id":"1234","target_id":5,"target_type":"Manga","chapters":12,"score":0,"status":"completed"}}"#,
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 3})))
        .expect(1)
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;

    let got = t.as_manga().unwrap().update(&manga(1, 12.0, 12), true).await.unwrap();

    assert_eq!(got.status, 2);
}

#[tokio::test]
async fn t_31a_013_bind_not_found_adds_as_reading_with_read_chapters() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/mangas/5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(catalogue(json!(20))))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/user_rates"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_string(
            r#"{"user_rate":{"user_id":"1234","target_id":5,"target_type":"Manga","chapters":2,"score":0,"status":"watching"}}"#,
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 77})))
        .expect(1)
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;
    let mut m = manga(1, 2.0, 20);
    m.score = 5.0;

    t.as_manga().unwrap().bind(&mut m, true).await.unwrap();

    assert_eq!((m.status, m.score, m.library_id), (1, 0.0, Some(77)));
}

#[tokio::test]
async fn t_31a_013_bind_found_keeps_local_dates_and_private_and_forces_reading() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/mangas/5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(catalogue(json!(20))))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/user_rates"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": 9, "chapters": 4.0, "episodes": 0.0, "score": 6, "status": "on_hold"}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_string(
            r#"{"user_rate":{"user_id":"1234","target_id":5,"target_type":"Manga","chapters":4,"score":6,"status":"watching"}}"#,
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 9})))
        .expect(1)
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;
    let mut m = manga(1, 0.0, 20);
    m.start_date = 1_700_000_000_000;
    m.finish_date = 1_700_000_100_000;
    m.private = true;

    t.as_manga().unwrap().bind(&mut m, true).await.unwrap();

    assert_eq!((m.status, m.score, m.last_chapter_read), (1, 6.0, 4.0));
    assert_eq!(m.library_id, Some(9));
    assert_eq!((m.start_date, m.finish_date), (1_700_000_000_000, 1_700_000_100_000));
    assert!(m.private);
}

#[tokio::test]
async fn t_31a_013_bind_found_completed_stays_completed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/animes/5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(catalogue(json!(20))))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/user_rates"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": 9, "chapters": 0.0, "episodes": 12.0, "score": 0, "status": "completed"}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 9})))
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;
    let mut a = anime(1, 0.0, 12);

    t.as_anime().unwrap().bind(&mut a, true).await.unwrap();

    assert_eq!(a.status, 2);
}

#[tokio::test]
async fn t_31a_013_a_found_entry_without_a_count_is_a_decode_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/mangas/5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(catalogue(json!(null))))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/user_rates"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": 9, "chapters": 0.0, "episodes": 0.0, "score": 0, "status": "watching"}
        ])))
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;

    let err = t.api().find_library(Kind::Manga, 5, "1234").await.unwrap_err();

    assert!(matches!(err, ShikimoriError::Decode(_)));
}

// ---- T-31a-001 delete path ----

#[tokio::test]
async fn t_31a_013_delete_needs_a_library_id_and_sends_the_delete() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/user_rates/9"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;
    let deletable = t.as_deletable_manga().unwrap();

    assert!(matches!(
        deletable.delete(&manga(1, 0.0, 0)).await,
        Err(TrackError::Key(_))
    ));
    let mut with_id = manga(1, 0.0, 0);
    with_id.library_id = Some(9);
    deletable.delete(&with_id).await.unwrap();
}

#[tokio::test]
async fn t_31a_009_unknown_status_on_a_user_rate_is_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/mangas/5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(catalogue(json!(20))))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/user_rates"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": 9, "chapters": 0.0, "episodes": 0.0, "score": 0, "status": "later"}
        ])))
        .mount(&server)
        .await;
    let (t, _) = logged_in(&server).await;

    assert_eq!(
        t.api().find_library(Kind::Manga, 5, "1234").await,
        Err(ShikimoriError::UnknownStatus("later".into()))
    );
}
