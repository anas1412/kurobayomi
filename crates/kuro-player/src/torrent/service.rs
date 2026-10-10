//! The torrent server as the player uses it (T-45a-071 to T-45a-079): starting it, the `Torrent Server is running`
//! notice, routing a video through it, and stopping it from the notice. The pure parts live in the sibling modules;
//! this one joins them in the order of `PlayerActivity.setVideo` and `TorrentServerService`.
//!
//! The tracker list is kept by the service and added to every magnet link as `&tr=` (area 13 section 5, option b),
//! because the child binary has no call that takes a tracker list.

use crate::local_server::{LocalServer, uses_local_server};
use crate::mpv::{PropertySink, PropertySource};
use crate::notices::{
    ACTION_HTTP_SERVER_CLOSE, ACTION_TORRENT_SERVER_STOP, HTTP_SERVER_NOTICE_ID, NoticeOp,
    TORRENT_SERVER_NOTICE_ID, http_server_running, torrent_server_running,
};
use crate::session::{LoadError, Session};
use crate::types::Video;
use crate::window::CommandSink;

use super::api::{TorrentApi, TorrentError};
use super::links::play_link;
use super::route::{TorrentSource, classify, is_torrent_route, magnet_index};
use super::server::{ServerConfig, ServerError, ServerState, TorrentServer};
use super::trackers::with_trackers;

#[derive(Debug, thiserror::Error)]
pub enum RouteError {
    #[error(transparent)]
    Server(#[from] ServerError),
    #[error(transparent)]
    Client(#[from] TorrentError),
    #[error("could not read the torrent file: {0}")]
    Read(String),
}

/// The torrent server owned by the app, with the tracker list pushed at its start.
#[derive(Default)]
pub struct TorrentService {
    server: TorrentServer,
    /// `pref_torrserver_tackers`, one tracker per line, as stored.
    trackers: String,
}

impl TorrentService {
    pub fn state(&self) -> &ServerState {
        self.server.state()
    }

    /// Start: an answering server is used as it is, otherwise the child starts and is awaited for at most 10 s. Then
    /// the tracker list is taken over and the running notice is returned.
    pub async fn start(
        &mut self,
        cfg: &ServerConfig,
        stored_trackers: &str,
    ) -> Result<Vec<NoticeOp>, ServerError> {
        self.server.ensure_running(cfg).await?;
        self.trackers = stored_trackers.to_owned();
        Ok(vec![NoticeOp::Show(torrent_server_running())])
    }

    /// Stop: ends the server and withdraws the notice.
    pub async fn stop(&mut self) -> Vec<NoticeOp> {
        self.server.stop().await;
        vec![NoticeOp::Withdraw(TORRENT_SERVER_NOTICE_ID)]
    }

    /// The URL mpv loads for `url`, or `None` when the video does not go through the torrent server (T-45a-078). A
    /// routed video starts the server first. A `content://` source is uploaded and plays at index 0 (T-45a-074); a
    /// magnet plays its `index=` file; any other link is fetched by the server and plays at index 0.
    pub async fn route(
        &mut self,
        url: &str,
        title: &str,
        enabled: bool,
        cfg: &ServerConfig,
        stored_trackers: &str,
    ) -> Result<Option<(String, Vec<NoticeOp>)>, RouteError> {
        let base = format!("http://127.0.0.1:{}", cfg.port);
        if !is_torrent_route(url, enabled, &base) {
            return Ok(None);
        }
        let ops = self.start(cfg, stored_trackers).await?;
        let api = TorrentApi::on_port(cfg.port)?;
        let (torrent, index) = match classify(url) {
            TorrentSource::Upload => {
                let path = url.strip_prefix("content://").unwrap_or(url);
                let bytes = std::fs::read(path).map_err(|e| RouteError::Read(e.to_string()))?;
                (api.upload(&bytes, title, false).await?, 0)
            }
            TorrentSource::Magnet => {
                let link = with_trackers(url, &self.trackers);
                (api.add(&link, title, false).await?, magnet_index(url))
            }
            TorrentSource::Url => (api.add(url, title, false).await?, 0),
        };
        Ok(Some((play_link(cfg.port, &torrent, index), ops)))
    }
}

/// What a routed load needs from the app: the service and the preferences it reads.
pub struct TorrentRoute<'a> {
    pub service: &'a mut TorrentService,
    pub enabled: bool,
    pub cfg: &'a ServerConfig,
    pub trackers: &'a str,
}

#[derive(Debug, thiserror::Error)]
pub enum RoutedLoadError {
    #[error(transparent)]
    Torrent(#[from] RouteError),
    #[error(transparent)]
    Load(#[from] LoadError),
}

impl<H: PropertySink + PropertySource + CommandSink> Session<H> {
    /// `PlayerActivity.setVideo`: a video that matches the torrent route is resolved through the torrent server, any
    /// other video loads as before. Returns the notices to show: `Torrent Server is running` after a torrent start,
    /// `Http server is running` while the local stream server serves this video.
    pub async fn load_video_routed(
        &mut self,
        video: &Video,
        server: &mut LocalServer,
        lookup: impl FnOnce() -> Option<std::sync::Arc<dyn kuro_source_http::local_server::LocalHandler>>,
        torrent: Option<TorrentRoute<'_>>,
    ) -> Result<Vec<NoticeOp>, RoutedLoadError> {
        let mut ops = Vec::new();
        let mut video = video.clone();
        if let Some(route) = torrent
            && let Some((url, started)) = route
                .service
                .route(&video.video_url, &video.video_title, route.enabled, route.cfg, route.trackers)
                .await?
        {
            video.video_url = url;
            ops = started;
        }
        self.load_video(&video, server, lookup)?;
        if uses_local_server(&video.video_url) && server.port().is_some() {
            ops.push(NoticeOp::Show(http_server_running()));
        }
        Ok(ops)
    }
}

/// Routes a notice button back to its server: `http_server.close` stops the local stream server and
/// `torrent_server.stop` stops the torrent server. Both return the notice to withdraw. An unknown action does nothing.
pub async fn on_notice_action(
    action: &str,
    local: &mut LocalServer,
    torrent: &mut TorrentService,
) -> Vec<NoticeOp> {
    match action {
        ACTION_HTTP_SERVER_CLOSE => {
            local.stop();
            vec![NoticeOp::Withdraw(HTTP_SERVER_NOTICE_ID)]
        }
        ACTION_TORRENT_SERVER_STOP => torrent.stop().await,
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::mpv::MpvError;
    use crate::torrent::proxy::ProxyMode;

    const TORRENT_JSON: &str = r#"{"title":"Show","hash":"abc","file_stats":[{"id":1,"path":"S/1.mkv","length":5},{"id":3,"path":"S/3.mkv","length":5}]}"#;

    fn config(port: u16) -> ServerConfig {
        ServerConfig {
            binary: Some(PathBuf::from("/nonexistent/torrserver")),
            port,
            data_dir: PathBuf::from("/data/torrserver"),
            proxy_mode: ProxyMode::None,
            proxy_url: String::new(),
        }
    }

    async fn running_server() -> MockServer {
        let server = MockServer::start().await;
        for (verb, at) in [("GET", "/echo"), ("POST", "/torrents"), ("POST", "/torrent/upload")] {
            Mock::given(method(verb))
                .and(path(at))
                .respond_with(ResponseTemplate::new(200).set_body_string(if at == "/echo" {
                    "1.0"
                } else {
                    TORRENT_JSON
                }))
                .mount(&server)
                .await;
        }
        server
    }

    #[tokio::test]
    async fn t_45a_078_a_plain_video_is_not_routed_and_starts_nothing() {
        let server = running_server().await;
        let cfg = config(server.address().port());
        let mut service = TorrentService::default();
        let routed = service.route("https://h.org/a.m3u8", "t", true, &cfg, "").await.unwrap();
        assert!(routed.is_none());
        assert_eq!(service.state(), &ServerState::Stopped);
        let off = service.route("magnet:?xt=urn:btih:a", "t", false, &cfg, "").await.unwrap();
        assert!(off.is_none(), "torrents disabled");
    }

    #[tokio::test]
    async fn t_45a_074_a_magnet_plays_its_index_and_shows_the_running_notice() {
        let server = running_server().await;
        let port = server.address().port();
        let mut service = TorrentService::default();
        let (url, ops) = service
            .route("magnet:?xt=urn:btih:a&index=3", "Show", true, &config(port), "udp://t:1")
            .await
            .unwrap()
            .expect("routed");
        assert_eq!(url, format!("http://127.0.0.1:{port}/stream/3.mkv?link=abc&index=3&play"));
        assert_eq!(ops, vec![NoticeOp::Show(torrent_server_running())]);
        assert_eq!(service.state(), &ServerState::Running { port });
        let body = String::from_utf8_lossy(&server.received_requests().await.unwrap().last().unwrap().body).into_owned();
        assert!(body.contains("tr=udp%3A%2F%2Ft%3A1"), "the tracker list is added: {body}");
    }

    #[tokio::test]
    async fn t_45a_074_a_non_integer_index_plays_file_zero_and_a_content_source_is_uploaded() {
        let server = running_server().await;
        let port = server.address().port();
        let mut service = TorrentService::default();
        let (url, _) = service
            .route("magnet:?xt=urn:btih:a&index=x", "Show", true, &config(port), "")
            .await
            .unwrap()
            .unwrap();
        assert!(url.contains("&index=0&"), "{url}");

        let file = std::env::temp_dir().join(format!("kuro-route-{}.torrent", std::process::id()));
        std::fs::write(&file, b"d4:name").unwrap();
        let source = format!("content://{}", file.display());
        let (url, _) = service.route(&source, "Show", true, &config(port), "").await.unwrap().unwrap();
        std::fs::remove_file(&file).unwrap();
        assert!(url.contains("&index=0&"), "{url}");
        let last = server.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(last.url.path(), "/torrent/upload");
    }

    #[tokio::test]
    async fn t_45a_071_and_079_the_notice_actions_stop_their_servers_and_withdraw_the_notice() {
        let server = running_server().await;
        let port = server.address().port();
        let mut torrent = TorrentService::default();
        torrent.start(&config(port), "").await.unwrap();
        let mut local = LocalServer::default();
        let ops = on_notice_action(ACTION_TORRENT_SERVER_STOP, &mut local, &mut torrent).await;
        assert_eq!(ops, vec![NoticeOp::Withdraw(TORRENT_SERVER_NOTICE_ID)]);
        assert_eq!(torrent.state(), &ServerState::Stopped);
        let ops = on_notice_action(ACTION_HTTP_SERVER_CLOSE, &mut local, &mut torrent).await;
        assert_eq!(ops, vec![NoticeOp::Withdraw(HTTP_SERVER_NOTICE_ID)]);
        assert!(on_notice_action("other", &mut local, &mut torrent).await.is_empty());
    }

    #[derive(Default)]
    struct Fake {
        commands: RefCell<Vec<Vec<String>>>,
    }
    impl PropertySink for Fake {
        fn set_str(&self, _: &str, _: &str) -> Result<(), MpvError> {
            Ok(())
        }
    }
    impl PropertySource for Fake {
        fn get_str(&self, _: &str) -> Option<String> {
            None
        }
    }
    impl CommandSink for Fake {
        fn command(&self, args: &[&str]) -> Result<(), MpvError> {
            self.commands.borrow_mut().push(args.iter().map(|a| (*a).to_owned()).collect());
            Ok(())
        }
    }

    fn video(url: &str) -> Video {
        Video { video_url: url.to_owned(), video_title: "Show".to_owned(), ..Video::default() }
    }

    #[tokio::test]
    async fn t_45a_078_a_routed_video_loads_the_play_link_in_mpv() {
        let server = running_server().await;
        let port = server.address().port();
        let cfg = config(port);
        let mut service = TorrentService::default();
        let mut session = Session::new(Fake::default());
        let mut local = LocalServer::default();
        let route = TorrentRoute { service: &mut service, enabled: true, cfg: &cfg, trackers: "" };
        let ops = session
            .load_video_routed(&video("magnet:?xt=urn:btih:a"), &mut local, || None, Some(route))
            .await
            .unwrap();
        assert_eq!(ops, vec![NoticeOp::Show(torrent_server_running())]);
        let sent = session.host.commands.borrow();
        let load = sent.iter().find(|c| c[0] == "loadfile").expect("loadfile");
        assert!(load[1].starts_with(&format!("http://127.0.0.1:{port}/stream/")), "{load:?}");
    }
}
