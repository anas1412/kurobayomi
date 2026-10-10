//! Wire tests of the Suwayomi client against a local mock server: the paths, the headers, the chapter write and the
//! settings read per call (T-33a-092 to T-33a-094; E-52 to E-63).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use wiremock::matchers::{body_string, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::api::{KEY_LOGIN, KEY_PASSWORD, KEY_SERVER_URL, SuwayomiApi};
use super::tests::tracker_with;
use super::{COMPLETED, READING, UNREAD};
use crate::model::{MangaTrack, TrackerId};
use crate::tracker::MangaTracker;
use crate::trackers::enhanced::EnhancedError;
use crate::trackers::http::tracker_client;
use crate::trackers::sources::memory::MemDirectory;
use crate::trackers::sources::{SourceDirectory, SourcePrefs, generate_source_id};

const MANGA: &str = include_str!("../../../tests/fixtures/suwayomi/manga_full.json");
const UNREAD_NULL: &str = include_str!("../../../tests/fixtures/suwayomi/manga_unread_null.json");
const CHAPTERS: &str = include_str!("../../../tests/fixtures/suwayomi/chapters.json");

/// A directory whose preferences can change between calls (E-63).
struct Live(Mutex<HashMap<String, String>>);

struct Snapshot(HashMap<String, String>);

impl SourcePrefs for Snapshot {
    fn get_string(&self, key: &str, default: &str) -> String {
        self.0
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_owned())
    }
}

impl SourceDirectory for Live {
    fn source_prefs(&self, _source_id: i64) -> Option<Arc<dyn SourcePrefs>> {
        Some(Arc::new(Snapshot(self.0.lock().unwrap().clone())))
    }
}

fn api_with(dir: Arc<dyn SourceDirectory>) -> SuwayomiApi {
    SuwayomiApi::new(tracker_client().unwrap(), dir)
}

fn installed(url: &str, login: &str, password: &str) -> MemDirectory {
    let mut dir = MemDirectory::default();
    dir.install(
        "tachidesk",
        "en",
        &[
            (KEY_SERVER_URL, url),
            (KEY_LOGIN, login),
            (KEY_PASSWORD, password),
        ],
    );
    dir
}

fn track(url: &str, last: f64) -> MangaTrack {
    MangaTrack {
        id: Some(1),
        manga_id: 1,
        tracker_id: TrackerId::SUWAYOMI,
        remote_id: 0,
        library_id: None,
        title: "T".into(),
        last_chapter_read: last,
        total_chapters: 0,
        status: READING,
        score: 3.0,
        remote_url: url.into(),
        start_date: 0,
        finish_date: 0,
        private: false,
    }
}

async fn get(server: &MockServer, p: &str, body: &str) {
    Mock::given(method("GET"))
        .and(path(p))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_owned()))
        .mount(server)
        .await;
}

#[tokio::test]
async fn t_33a_092_e52_a_numeric_url_without_a_server_address_is_not_configured() {
    let api = api_with(Arc::new(MemDirectory::default()));
    let err = api.get_track_search("123").await.unwrap_err();
    assert!(matches!(err, EnhancedError::SourceNotConfigured));
}

#[tokio::test]
async fn t_33a_092_e53_a_non_numeric_url_is_used_verbatim() {
    let server = MockServer::start().await;
    get(&server, "/api/v1/manga/123/full", MANGA).await;
    let url = format!("{}/api/v1/manga/123", server.uri());
    let api = api_with(Arc::new(MemDirectory::default()));
    let found = api.get_track_search(&url).await.unwrap();
    assert_eq!(found.tracking_url, url);
    assert_eq!(found.cover_url, format!("{url}/thumbnail"));
}

#[tokio::test]
async fn t_33a_092_e58_the_search_reads_the_manga_fields() {
    let server = MockServer::start().await;
    get(&server, "/m/full", MANGA).await;
    let api = api_with(Arc::new(MemDirectory::default()));
    let found = api
        .get_track_search(&format!("{}/m", server.uri()))
        .await
        .unwrap();
    assert_eq!(found.title, "Title");
    assert_eq!(found.summary, "Desc");
    assert_eq!(found.total_chapters, 10);
    assert_eq!(found.publishing_status, "ONGOING");
    assert_eq!(found.last_chapter_read, 5.0);
    assert_eq!(found.status, READING);
}

#[tokio::test]
async fn t_33a_092_e58_no_last_read_is_zero_and_null_unread_is_reading() {
    let server = MockServer::start().await;
    let mut v: serde_json::Value = serde_json::from_str(UNREAD_NULL).unwrap();
    v["lastChapterRead"] = serde_json::Value::Null;
    get(&server, "/m/full", &v.to_string()).await;
    let api = api_with(Arc::new(MemDirectory::default()));
    let found = api
        .get_track_search(&format!("{}/m", server.uri()))
        .await
        .unwrap();
    assert_eq!(found.last_chapter_read, 0.0);
    assert_eq!(found.status, READING);
}

#[tokio::test]
async fn t_33a_092_e62_basic_auth_is_sent_when_both_values_are_set() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/m/full"))
        .and(header("authorization", "Basic YTpi"))
        .respond_with(ResponseTemplate::new(200).set_body_string(MANGA))
        .expect(1)
        .mount(&server)
        .await;
    let api = api_with(Arc::new(installed("", "a", "b")));
    api.get_track_search(&format!("{}/m", server.uri()))
        .await
        .unwrap();
}

#[tokio::test]
async fn t_33a_091_e63_the_settings_are_read_again_on_every_call() {
    let server = MockServer::start().await;
    get(&server, "/api/v1/manga/7/full", MANGA).await;
    let dir = Arc::new(Live(Mutex::new(HashMap::new())));
    let api = api_with(dir.clone());
    assert!(matches!(
        api.get_track_search("7").await,
        Err(EnhancedError::SourceNotConfigured)
    ));
    dir.0
        .lock()
        .unwrap()
        .insert(KEY_SERVER_URL.into(), server.uri());
    let found = api.get_track_search("7").await.unwrap();
    assert_eq!(found.title, "Title");
}

#[tokio::test]
async fn t_33a_093_e59_the_chapter_write_uses_the_index_and_the_form_body() {
    let server = MockServer::start().await;
    let url = format!("{}/m", server.uri());
    get(&server, "/m/chapters", CHAPTERS).await;
    Mock::given(method("PUT"))
        .and(path("/m/chapter/7"))
        .and(body_string("markPrevRead=true&read=true"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    get(&server, "/m/full", MANGA).await;
    let api = api_with(Arc::new(MemDirectory::default()));
    let updated = api.update_progress(&track(&url, 5.0)).await.unwrap();
    assert_eq!(updated.total_chapters, 10);
    assert_eq!(updated.status, READING);
}

#[tokio::test]
async fn t_33a_093_e60_a_number_the_server_does_not_have_is_chapter_not_on_server() {
    let server = MockServer::start().await;
    let url = format!("{}/m", server.uri());
    get(&server, "/m/chapters", CHAPTERS).await;
    let api = api_with(Arc::new(MemDirectory::default()));
    let err = api.update_progress(&track(&url, 0.0)).await.unwrap_err();
    assert!(matches!(err, EnhancedError::ChapterNotOnServer(ref n) if n == "0.0"));
}

#[tokio::test]
async fn t_33a_093_e61_the_first_chapter_in_list_order_wins() {
    let server = MockServer::start().await;
    let url = format!("{}/m", server.uri());
    get(&server, "/m/chapters", CHAPTERS).await;
    Mock::given(method("PUT"))
        .and(path("/m/chapter/7"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    get(&server, "/m/full", MANGA).await;
    let api = api_with(Arc::new(MemDirectory::default()));
    api.update_progress(&track(&url, 5.0)).await.unwrap();
}

#[tokio::test]
async fn t_33a_081_refresh_copies_the_server_progress_and_keeps_the_score() {
    let server = MockServer::start().await;
    get(&server, "/m/full", MANGA).await;
    let tracker = tracker_with(MemDirectory::default());
    let local = track(&format!("{}/m", server.uri()), 1.0);
    let refreshed = tracker.refresh(&local).await.unwrap();
    assert_eq!(refreshed.last_chapter_read, 5.0);
    assert_eq!(refreshed.total_chapters, 10);
    assert_eq!(refreshed.score, 3.0);
}

#[tokio::test]
async fn t_33a_094_e54_a_fully_unread_manga_is_unread_on_the_wire() {
    let server = MockServer::start().await;
    let mut v: serde_json::Value = serde_json::from_str(MANGA).unwrap();
    v["unreadCount"] = serde_json::json!(10);
    get(&server, "/m/full", &v.to_string()).await;
    let api = api_with(Arc::new(MemDirectory::default()));
    let found = api
        .get_track_search(&format!("{}/m", server.uri()))
        .await
        .unwrap();
    assert_eq!(found.status, UNREAD);
    v["unreadCount"] = serde_json::json!(0);
    server.reset().await;
    get(&server, "/m/full", &v.to_string()).await;
    let found = api
        .get_track_search(&format!("{}/m", server.uri()))
        .await
        .unwrap();
    assert_eq!(found.status, COMPLETED);
}

#[test]
fn t_33a_082_the_source_id_is_the_tachidesk_slot() {
    assert_eq!(generate_source_id("tachidesk", "en", 1), 3100117499901280806);
}

fn entry(url: &str) -> crate::trackers::enhanced::EntryRef {
    crate::trackers::enhanced::EntryRef {
        id: 1,
        source_id: 5,
        url: url.into(),
        title: "T".into(),
    }
}

/// A Suwayomi tracker in the fake registry, with the extension's preferences installed.
async fn bind_harness(logged_in: bool, base: &str) -> crate::fakes::Harness {
    use crate::trackers::enhanced::MemFlags;
    let dir = installed(base, "", "");
    let t = Arc::new(
        super::SuwayomiTracker::new(Arc::new(dir), Arc::new(MemFlags::default())).unwrap(),
    );
    if logged_in {
        crate::tracker::Tracker::login(t.as_ref(), "", "").await.unwrap();
    }
    crate::fakes::Harness::with_tracker(Some(t))
}

#[tokio::test]
async fn t_33a_082_e94_a_logged_out_suwayomi_is_skipped_by_the_bind_path() {
    let server = MockServer::start().await;
    let h = bind_harness(false, &server.uri()).await;
    let url = format!("{}/api/v1/manga/9", server.uri());
    let failures =
        crate::bind::bind_enhanced_manga_entry(&h.ctx, &entry(&url), "tachidesk/en").await;
    assert!(failures.is_empty());
    assert!(h.ctx.manga_tracks.list_for_entry(1).await.unwrap().is_empty());
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn t_33a_082_e95_a_source_suwayomi_does_not_accept_is_skipped_by_the_bind_path() {
    let server = MockServer::start().await;
    let h = bind_harness(true, &server.uri()).await;
    let url = format!("{}/api/v1/manga/9", server.uri());
    let failures =
        crate::bind::bind_enhanced_manga_entry(&h.ctx, &entry(&url), "komga/all").await;
    assert!(failures.is_empty());
    assert!(h.ctx.manga_tracks.list_for_entry(1).await.unwrap().is_empty());
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn t_33a_101_the_bind_path_matches_by_entry_url_and_stores_the_binding() {
    let server = MockServer::start().await;
    get(&server, "/api/v1/manga/9/full", MANGA).await;
    let h = bind_harness(true, &server.uri()).await;
    let url = format!("{}/api/v1/manga/9", server.uri());
    let failures =
        crate::bind::bind_enhanced_manga_entry(&h.ctx, &entry(&url), "tachidesk/en").await;
    assert!(failures.is_empty(), "{failures:?}");
    let rows = h.ctx.manga_tracks.list_for_entry(1).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].remote_url, url);
}
