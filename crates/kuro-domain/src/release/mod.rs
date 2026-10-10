//! Application release check (`GetApplicationRelease.kt`, `ReleaseServiceImpl.kt`, `GithubRelease.kt`): the latest
//! GitHub release, the version rule, the three-day throttle, and the `@mention` linkifier of the release notes.
//!
//! Dropped from upstream: the in-app APK download and install (the installer is Android only) and the preview update
//! channel (there is no preview build type). The user loses the one-tap in-app install; the distro package manager is
//! the update path. The dialog offers the release page and "Not now".

pub mod check;
pub mod github;
pub mod mentions;
pub mod version;

/// One release as the check reads it (`Release.kt`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// The tag, such as `v1.2.0`.
    pub version: String,
    /// The release notes (`body`), empty when GitHub sends none.
    pub info: String,
    /// The release page (`html_url`).
    pub release_link: String,
    /// The download of the asset this host should use.
    pub download_link: String,
}

/// Failure of a release lookup. The check leaves its last-check time unchanged on any of these.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum ReleaseError {
    /// The server answered with a status outside 2xx (a rate limit is a 403).
    #[error("HTTP {0}")]
    Http(u16),
    /// No response: the request could not be made or did not complete.
    #[error("network error: {0}")]
    Transport(String),
    /// The body is not a release document.
    #[error("invalid release response: {0}")]
    Parse(String),
}

/// `ReleaseService.latest`. `Ok(None)` when the latest release has no asset for this host.
pub trait ReleaseService {
    fn latest(&self, repository: &str) -> Result<Option<Release>, ReleaseError>;
}
