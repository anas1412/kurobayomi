//! The GitHub latest-release client (`ReleaseServiceImpl.latest`, `GithubRelease.kt`). The HTTP call is behind
//! [`ReleaseHttp`], so the app wires its own transport and tests can answer with fixed bodies.

use serde::Deserialize;

use super::{Release, ReleaseError, ReleaseService};

/// Root of the GitHub REST API.
pub const API_ROOT: &str = "https://api.github.com";

/// User-Agent sent with every release request. GitHub refuses requests without one.
pub const USER_AGENT: &str = "Kurobayomi";

/// The one network call of the client: a GET with the given User-Agent. Returns the status and the body. A transport
/// failure (no response at all) is the `Err` text.
pub trait ReleaseHttp {
    fn get(&self, url: &str, user_agent: &str) -> Result<(u16, Vec<u8>), String>;
}

/// `https://api.github.com/repos/<repository>/releases/latest`.
pub fn latest_url(repository: &str) -> String {
    format!("{API_ROOT}/repos/{repository}/releases/latest")
}

#[derive(Deserialize)]
struct ReleaseDto {
    tag_name: String,
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    assets: Vec<AssetDto>,
}

#[derive(Deserialize)]
struct AssetDto {
    name: String,
    browser_download_url: String,
}

/// The names of a Linux build of `arch` (the host architecture as Rust names it), lower case.
fn arch_tokens(arch: &str) -> &[&str] {
    match arch {
        "x86_64" => &["x86_64", "amd64"],
        "aarch64" => &["aarch64", "arm64"],
        _ => &[],
    }
}

/// The asset of this host: the first whose lower-case name holds `linux` and an architecture token of `arch`. `None`
/// when no asset matches (TC-REL-04).
pub fn choose_asset<'a>(assets: &'a [(String, String)], arch: &str) -> Option<&'a (String, String)> {
    let tokens = arch_tokens(arch);
    assets.iter().find(|(name, _)| {
        let lower = name.to_ascii_lowercase();
        lower.contains("linux") && tokens.iter().any(|t| lower.contains(t))
    })
}

/// The GitHub release source. `arch` is the host architecture (`std::env::consts::ARCH` in the app).
pub struct GithubReleases<H: ReleaseHttp> {
    http: H,
    arch: String,
}

impl<H: ReleaseHttp> GithubReleases<H> {
    pub fn new(http: H, arch: &str) -> Self {
        Self { http, arch: arch.to_owned() }
    }
}

impl<H: ReleaseHttp> ReleaseService for GithubReleases<H> {
    fn latest(&self, repository: &str) -> Result<Option<Release>, ReleaseError> {
        let (status, body) = self.http.get(&latest_url(repository), USER_AGENT).map_err(ReleaseError::Transport)?;
        if !(200..300).contains(&status) {
            return Err(ReleaseError::Http(status));
        }
        let dto: ReleaseDto = serde_json::from_slice(&body).map_err(|e| ReleaseError::Parse(e.to_string()))?;
        let assets: Vec<(String, String)> = dto.assets.into_iter().map(|a| (a.name, a.browser_download_url)).collect();
        Ok(choose_asset(&assets, &self.arch).map(|(_, url)| Release {
            version: dto.tag_name,
            info: dto.body.unwrap_or_default(),
            release_link: dto.html_url,
            download_link: url.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// Answers every GET with one fixed reply and records the URL and the User-Agent it was asked with.
    struct Fixed {
        reply: Result<(u16, Vec<u8>), String>,
        seen: RefCell<Vec<(String, String)>>,
    }

    impl Fixed {
        fn new(reply: Result<(u16, Vec<u8>), String>) -> Self {
            Self { reply, seen: RefCell::new(Vec::new()) }
        }
    }

    impl ReleaseHttp for Fixed {
        fn get(&self, url: &str, user_agent: &str) -> Result<(u16, Vec<u8>), String> {
            self.seen.borrow_mut().push((url.to_owned(), user_agent.to_owned()));
            self.reply.clone()
        }
    }

    const BODY: &str = r#"{
        "tag_name": "v1.2.0",
        "body": null,
        "html_url": "https://github.com/o/r/releases/tag/v1.2.0",
        "assets": [
            {"name": "Kurobayomi-windows-x86_64.zip", "browser_download_url": "https://dl/win"},
            {"name": "Kurobayomi-linux-x86_64.AppImage", "browser_download_url": "https://dl/linux"}
        ]
    }"#;

    fn client(reply: Result<(u16, Vec<u8>), String>, arch: &str) -> GithubReleases<Fixed> {
        GithubReleases::new(Fixed::new(reply), arch)
    }

    #[test]
    fn the_request_goes_to_the_latest_endpoint_with_a_user_agent() {
        let c = client(Ok((200, BODY.as_bytes().to_vec())), "x86_64");
        c.latest("o/r").unwrap();
        assert_eq!(c.http.seen.borrow().as_slice(), [("https://api.github.com/repos/o/r/releases/latest".to_owned(), USER_AGENT.to_owned())]);
    }

    #[test]
    fn the_release_takes_the_tag_notes_page_and_the_linux_asset() {
        let got = client(Ok((200, BODY.as_bytes().to_vec())), "x86_64").latest("o/r").unwrap().unwrap();
        assert_eq!(got.version, "v1.2.0");
        assert_eq!(got.info, "", "a null body is empty notes");
        assert_eq!(got.release_link, "https://github.com/o/r/releases/tag/v1.2.0");
        assert_eq!(got.download_link, "https://dl/linux");
    }

    #[test]
    fn no_asset_for_this_host_is_none() {
        assert_eq!(client(Ok((200, BODY.as_bytes().to_vec())), "aarch64").latest("o/r").unwrap(), None);
    }

    #[test]
    fn a_non_2xx_status_is_an_http_error_with_the_code() {
        assert_eq!(client(Ok((403, Vec::new())), "x86_64").latest("o/r"), Err(ReleaseError::Http(403)));
    }

    #[test]
    fn a_transport_failure_is_an_error_and_a_bad_body_is_a_parse_error() {
        assert!(matches!(client(Err("refused".into()), "x86_64").latest("o/r"), Err(ReleaseError::Transport(_))));
        assert!(matches!(client(Ok((200, b"<html>".to_vec())), "x86_64").latest("o/r"), Err(ReleaseError::Parse(_))));
    }

    #[test]
    fn the_asset_rule_needs_linux_and_the_architecture_in_the_name() {
        let assets = vec![
            ("app-windows-x86_64.zip".to_owned(), "w".to_owned()),
            ("app-linux-arm64.tar.gz".to_owned(), "a".to_owned()),
            ("app-Linux-AMD64.tar.gz".to_owned(), "b".to_owned()),
        ];
        assert_eq!(choose_asset(&assets, "x86_64").map(|a| a.1.as_str()), Some("b"));
        assert_eq!(choose_asset(&assets, "aarch64").map(|a| a.1.as_str()), Some("a"));
        assert_eq!(choose_asset(&assets, "riscv64"), None);
        assert_eq!(choose_asset(&assets[..1], "x86_64"), None);
    }
}
