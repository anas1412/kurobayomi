//! The GitHub latest-release client (`ReleaseServiceImpl.latest`, `GithubRelease.kt`). The HTTP call is behind
//! [`ReleaseHttp`], so the app wires its own transport and tests can answer with fixed bodies.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
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

/// The architecture tags an asset name can carry as `-<tag>`, as Rust names them.
const ARCH_TAGS: [&str; 2] = ["x86_64", "aarch64"];

/// The download of this host (`getDownloadLink`): each asset is keyed by the architecture tag its name carries as
/// `-<tag>` (or no tag), a later asset replacing an earlier one with the same key, and the answer is the entry for
/// `arch`, else the one with no tag. `None` when neither exists (TC-REL-04).
pub fn pick_download_link<'a>(assets: &'a [(String, String)], arch: &str) -> Option<&'a str> {
    let mut by_tag: HashMap<Option<&str>, &str> = HashMap::new();
    for (name, url) in assets {
        let tag = ARCH_TAGS.into_iter().find(|t| name.contains(&format!("-{t}")));
        by_tag.insert(tag, url);
    }
    by_tag.get(&Some(arch)).or_else(|| by_tag.get(&None)).copied()
}

/// The release notes without the checksum footer (`(?s)---.*Checksums.*`).
fn strip_checksums(info: &str) -> String {
    static FOOTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)---.*Checksums.*").expect("fixed pattern"));
    FOOTER.replace_all(info, "").into_owned()
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

    /// The client for the host this build runs on: `std::env::consts::ARCH` in place of `Build.SUPPORTED_ABIS[0]`.
    pub fn for_this_host(http: H) -> Self {
        Self::new(http, std::env::consts::ARCH)
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
        Ok(pick_download_link(&assets, &self.arch).map(|url| Release {
            version: dto.tag_name,
            info: strip_checksums(&dto.body.unwrap_or_default()),
            release_link: dto.html_url,
            download_link: url.to_owned(),
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
    fn for_this_host_reads_the_build_architecture() {
        let host = GithubReleases::for_this_host(Fixed::new(Ok((200, BODY.as_bytes().to_vec()))));
        assert_eq!(host.arch, std::env::consts::ARCH);
        // BODY's only Linux asset is for x86_64, so only an x86_64 build finds it.
        assert_eq!(host.latest("o/r").unwrap().is_some(), std::env::consts::ARCH == "x86_64");
    }

    fn assets(names: &[&str]) -> Vec<(String, String)> {
        names.iter().map(|n| ((*n).to_owned(), format!("https://dl/{n}"))).collect()
    }

    fn pick(names: &[&str], arch: &str) -> Option<String> {
        pick_download_link(&assets(names), arch).map(|u| u.trim_start_matches("https://dl/").to_owned())
    }

    #[test]
    fn the_tagged_asset_of_this_architecture_is_picked() {
        let both = ["k-1.0-x86_64.tar.zst", "k-1.0-aarch64.tar.zst"];
        assert_eq!(pick(&both, "x86_64").as_deref(), Some("k-1.0-x86_64.tar.zst"));
        assert_eq!(pick(&both, "aarch64").as_deref(), Some("k-1.0-aarch64.tar.zst"));
    }

    #[test]
    fn an_untagged_asset_is_the_fallback_and_a_tagged_one_beats_it() {
        assert_eq!(pick(&["k-1.0.tar.zst"], "x86_64").as_deref(), Some("k-1.0.tar.zst"));
        assert_eq!(pick(&["k-1.0.tar.zst", "k-1.0-x86_64.tar.zst"], "x86_64").as_deref(), Some("k-1.0-x86_64.tar.zst"));
        assert_eq!(pick(&["k-1.0-x86_64.tar.zst", "k-1.0.tar.zst"], "x86_64").as_deref(), Some("k-1.0-x86_64.tar.zst"));
        assert_eq!(pick(&["k-1.0.tar.zst", "k-1.0-x86_64.tar.zst"], "riscv64").as_deref(), Some("k-1.0.tar.zst"));
    }

    #[test]
    fn only_another_architecture_or_no_assets_is_none() {
        assert_eq!(pick(&["k-1.0-aarch64.tar.zst"], "x86_64"), None);
        assert_eq!(pick(&[], "x86_64"), None);
    }

    #[test]
    fn the_later_asset_wins_on_a_duplicate_key() {
        let dup = [("a-x86_64.zip", "first"), ("b-x86_64.zip", "last")].map(|(n, u)| (n.to_owned(), u.to_owned()));
        assert_eq!(pick_download_link(&dup, "x86_64"), Some("last"));
        let untagged = [("a.zip", "first"), ("b.zip", "last")].map(|(n, u)| (n.to_owned(), u.to_owned()));
        assert_eq!(pick_download_link(&untagged, "x86_64"), Some("last"));
    }

    #[test]
    fn the_android_abi_names_are_not_architecture_tags() {
        assert_eq!(pick(&["app-arm64-v8a.apk"], "aarch64").as_deref(), Some("app-arm64-v8a.apk"), "untagged, so the fallback");
        assert_eq!(pick(&["app-arm64-v8a.apk", "app-aarch64.apk"], "aarch64").as_deref(), Some("app-aarch64.apk"));
    }

    #[test]
    fn the_checksum_footer_is_cut_from_the_notes() {
        assert_eq!(strip_checksums("Fixes.\n\n---\n## Checksums\nabc  file\n"), "Fixes.\n\n");
        assert_eq!(strip_checksums("Fixes\n---\nmore"), "Fixes\n---\nmore", "a rule with no Checksums after it stays");
        assert_eq!(strip_checksums("No footer"), "No footer");
    }

    #[test]
    fn the_release_notes_come_back_without_the_footer() {
        let body = r#"{"tag_name":"v1","body":"Notes\n---\nChecksums\nabc","html_url":"u",
            "assets":[{"name":"k-x86_64.zip","browser_download_url":"d"}]}"#;
        let got = client(Ok((200, body.as_bytes().to_vec())), "x86_64").latest("o/r").unwrap().unwrap();
        assert_eq!(got.info, "Notes\n");
    }
}
