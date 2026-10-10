//! Lifecycle of the torrent server (T-45a-072; area 13 sections 4, 6 and 9). The server is a child process that binds
//! loopback only. The app owns it: `stop` runs at shutdown, and the child is killed when it is dropped. The caller
//! serialises `ensure_running` and `stop`, so no single-flight lock is needed here.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use super::api::{TorrentApi, TorrentError};
use super::proxy::{ProxyMode, start_arguments, valid_proxy_url};

/// The port when the preference is missing or unusable (`pref_torrserver_port`, default `8090`).
pub const DEFAULT_PORT: u16 = 8090;
/// Readiness probes after the start, one per second (area 13 section 4.2, `wait(10)`): ten probes with a one second pause after each, so at most 10 s.
const READY_PROBES: u32 = 10;
/// SIGTERM waits this long before the child is killed (area 13 section 4.3).
const STOP_GRACE: Duration = Duration::from_secs(5);
/// The program looked up on `PATH` when no binary path is configured.
const PROGRAM: &str = "torrserver";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ServerError {
    #[error("TorrServer binary not found")]
    BinaryMissing,
    #[error("invalid proxy url")]
    InvalidProxy,
    #[error("could not start TorrServer: {0}")]
    Spawn(String),
    #[error("TorrServer exited with code {0}")]
    Exited(String),
    #[error("did not respond within 10 s")]
    NotReady,
    #[error(transparent)]
    Client(#[from] TorrentError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerState {
    Stopped,
    Running { port: u16 },
    Failed(String),
}

/// What the server starts with. The app reads it from the preferences at each start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerConfig {
    /// The configured binary. `None` looks `torrserver` up on `PATH`.
    pub binary: Option<PathBuf>,
    pub port: u16,
    pub data_dir: PathBuf,
    pub proxy_mode: ProxyMode,
    pub proxy_url: String,
}

/// The stored port: a decimal in `1..=65535`. Anything else is `DEFAULT_PORT`. Port `0` (the OS picks) is not
/// supported yet, because the server would have to report the bound port.
pub fn port_from_pref(text: &str) -> u16 {
    text.trim()
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .unwrap_or(DEFAULT_PORT)
}

/// The child's arguments: port, loopback bind, data directory, and the proxy pair (area 13 section 4.1). The flag
/// names are the proposal; the spike (T-13a-041) checks them against the chosen binary.
pub fn child_args(cfg: &ServerConfig) -> Vec<String> {
    let (mode, proxy) = start_arguments(cfg.proxy_mode, &cfg.proxy_url);
    let mut args = vec![
        "--port".to_owned(),
        cfg.port.to_string(),
        "--bind".to_owned(),
        "127.0.0.1".to_owned(),
        "--data".to_owned(),
        cfg.data_dir.display().to_string(),
        "--proxy-mode".to_owned(),
        mode.to_owned(),
    ];
    if let Some(url) = proxy {
        args.push("--proxy-url".to_owned());
        args.push(url.to_owned());
    }
    args
}

/// The first file called `name` on `PATH`.
fn find_on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?
        .to_string_lossy()
        .split(':')
        .map(|dir| Path::new(dir).join(name))
        .find(|candidate| candidate.is_file())
}

/// The torrent server owned by the app. It is `Stopped` until `ensure_running` starts it.
#[derive(Debug)]
pub struct TorrentServer {
    state: ServerState,
    child: Option<Child>,
}

impl Default for TorrentServer {
    fn default() -> Self {
        Self {
            state: ServerState::Stopped,
            child: None,
        }
    }
}

impl TorrentServer {
    pub fn state(&self) -> &ServerState {
        &self.state
    }

    /// Returns the port of a running server. A server that already answers on the port is used as it is. Otherwise the
    /// child starts and is probed once a second until it answers. A failure leaves `Failed(reason)`.
    pub async fn ensure_running(&mut self, cfg: &ServerConfig) -> Result<u16, ServerError> {
        let api = TorrentApi::on_port(cfg.port)?;
        if !api.echo().await.is_empty() {
            self.state = ServerState::Running { port: cfg.port };
            return Ok(cfg.port);
        }
        match self.start_child(cfg, &api).await {
            Ok(()) => {
                self.state = ServerState::Running { port: cfg.port };
                Ok(cfg.port)
            }
            Err(error) => {
                self.kill().await;
                self.state = ServerState::Failed(error.to_string());
                Err(error)
            }
        }
    }

    async fn start_child(
        &mut self,
        cfg: &ServerConfig,
        api: &TorrentApi,
    ) -> Result<(), ServerError> {
        if cfg.proxy_mode != ProxyMode::None && !valid_proxy_url(&cfg.proxy_url) {
            return Err(ServerError::InvalidProxy);
        }
        let binary = cfg
            .binary
            .clone()
            .or_else(|| find_on_path(PROGRAM))
            .ok_or(ServerError::BinaryMissing)?;
        let mut child = Command::new(&binary)
            .args(child_args(cfg))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ServerError::Spawn(e.to_string()))?;
        if let Some(out) = child.stdout.take() {
            tokio::spawn(log_lines(out));
        }
        if let Some(err) = child.stderr.take() {
            tokio::spawn(log_lines(err));
        }
        self.child = Some(child);
        for _ in 0..READY_PROBES {
            if let Some(child) = self.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    let code = status
                        .code()
                        .map_or_else(|| "signal".to_owned(), |code| code.to_string());
                    return Err(ServerError::Exited(code));
                }
            }
            if !api.echo().await.is_empty() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Err(ServerError::NotReady)
    }

    /// Stops the child: SIGTERM, then SIGKILL after the grace period. Ends in `Stopped`.
    pub async fn stop(&mut self) {
        self.kill().await;
        self.state = ServerState::Stopped;
    }

    async fn kill(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        if let Some(pid) = child.id() {
            // SIGTERM without a libc binding: `kill` is part of every Linux base system.
            let _ = Command::new("kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }
        if tokio::time::timeout(STOP_GRACE, child.wait())
            .await
            .is_err()
        {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
    }
}

/// Sends each line of a child stream to the log under the `torrserver` target.
async fn log_lines(stream: impl tokio::io::AsyncRead + Unpin) {
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        tracing::info!(target: "torrserver", "{line}");
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn config(port: u16, binary: Option<PathBuf>) -> ServerConfig {
        ServerConfig {
            binary,
            port,
            data_dir: PathBuf::from("/data/torrserver"),
            proxy_mode: ProxyMode::None,
            proxy_url: String::new(),
        }
    }

    #[test]
    fn the_port_comes_from_a_valid_preference_or_the_default() {
        assert_eq!(port_from_pref("9000"), 9000);
        assert_eq!(port_from_pref(" 8091 "), 8091);
        assert_eq!(
            port_from_pref("0"),
            DEFAULT_PORT,
            "port 0 is not supported yet"
        );
        assert_eq!(port_from_pref("70000"), DEFAULT_PORT);
        assert_eq!(port_from_pref("abc"), DEFAULT_PORT);
    }

    #[test]
    fn the_child_binds_loopback_and_takes_the_proxy_pair_only_when_set() {
        let plain = child_args(&config(8090, None));
        assert!(plain.windows(2).any(|w| w == ["--bind", "127.0.0.1"]));
        assert!(plain.windows(2).any(|w| w == ["--proxy-mode", "tracker"]));
        assert!(!plain.iter().any(|a| a == "--proxy-url"));

        let mut cfg = config(8090, None);
        cfg.proxy_mode = ProxyMode::Full;
        cfg.proxy_url = "socks5://proxy.example:1080".into();
        let proxied = child_args(&cfg);
        assert!(proxied.windows(2).any(|w| w == ["--proxy-mode", "full"]));
        assert!(
            proxied
                .windows(2)
                .any(|w| w == ["--proxy-url", "socks5://proxy.example:1080"])
        );
    }

    #[tokio::test]
    async fn a_server_that_already_answers_is_used_without_a_child() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/echo"))
            .respond_with(ResponseTemplate::new(200).set_body_string("1.0"))
            .mount(&server)
            .await;
        let port = server.address().port();
        let mut torrent = TorrentServer::default();
        let cfg = config(port, Some(PathBuf::from("/nonexistent/torrserver")));
        assert_eq!(torrent.ensure_running(&cfg).await, Ok(port));
        assert_eq!(torrent.state(), &ServerState::Running { port });
    }

    #[tokio::test]
    async fn a_missing_binary_is_a_failed_state_with_the_reason() {
        let mut torrent = TorrentServer::default();
        let cfg = config(1, Some(PathBuf::from("/nonexistent/torrserver")));
        let err = torrent.ensure_running(&cfg).await.unwrap_err();
        assert!(matches!(err, ServerError::Spawn(_)), "{err:?}");
        assert!(matches!(torrent.state(), ServerState::Failed(_)));
    }

    #[tokio::test]
    async fn an_invalid_proxy_url_fails_before_any_child_starts() {
        let mut torrent = TorrentServer::default();
        let mut cfg = config(1, Some(PathBuf::from("/nonexistent/torrserver")));
        cfg.proxy_mode = ProxyMode::Peers;
        cfg.proxy_url = "ftp://nowhere".into();
        assert_eq!(
            torrent.ensure_running(&cfg).await,
            Err(ServerError::InvalidProxy)
        );
    }

    #[tokio::test]
    async fn stop_with_no_child_ends_stopped() {
        let mut torrent = TorrentServer::default();
        torrent.stop().await;
        assert_eq!(torrent.state(), &ServerState::Stopped);
    }
}
