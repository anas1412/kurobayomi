//! `AppAction` and its router (T-35a-006, T-35a-007, T-35a-014, T-35a-015, T-35a-020, T-35a-021): every notice button
//! and tap names an [`AppAction`], serialised as JSON into the string parameter of the `app.notice` action. [`route`]
//! checks the untrusted payload and calls an [`ActionHost`]; the host (in `notify`) holds the GTK and runtime half, so
//! the rules here are tested with a fake.
//!
//! Decisions:
//! - The router lives in `misc` because `src/notify` is not part of this lane. Notice ids are plain strings.
//! - A notice-carrying action withdraws its notice before the work starts, as the plan says.
//! - Ids at or below 0, more than 10 000 urls and a url longer than 2048 bytes are rejected with a warning.

use std::path::PathBuf;

use kuro_domain::kind::EntryKind;
use serde::{Deserialize, Serialize};

/// The `app.notice` action name without the `app.` prefix.
pub const NOTICE_ACTION_NAME: &str = "notice";
/// Largest payload accepted from the session bus (PROPOSAL, 1 MiB).
pub const MAX_PAYLOAD_BYTES: usize = 1 << 20;
const MAX_URLS: usize = 10_000;
const MAX_URL_BYTES: usize = 2048;
/// The toast of `notify.error.item_missing`.
pub const ITEM_MISSING_TOAST: &str = "Error";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppAction {
    OpenMangaDownloads,
    OpenAnimeDownloads,
    OpenChapter { manga_id: i64, chapter_id: i64 },
    OpenEpisode { anime_id: i64, episode_id: i64 },
    OpenManga { manga_id: i64, notice: Option<String> },
    OpenAnime { anime_id: i64, notice: Option<String> },
    MarkChaptersRead { manga_id: i64, urls: Vec<String>, notice: Option<String> },
    MarkEpisodesSeen { anime_id: i64, urls: Vec<String>, notice: Option<String> },
    DownloadChapters { manga_id: i64, urls: Vec<String>, notice: Option<String> },
    DownloadEpisodes { anime_id: i64, urls: Vec<String>, notice: Option<String> },
    ResumeMangaDownloads,
    PauseMangaDownloads,
    ClearMangaDownloads,
    ResumeAnimeDownloads,
    PauseAnimeDownloads,
    ClearAnimeDownloads,
    CancelLibraryUpdate { notice: Option<String> },
    CancelAnimeLibraryUpdate { notice: Option<String> },
    CancelRestore,
    /// `sha256` is the release digest when the release names one; the download is then hashed and checked (T-35a-054).
    StartAppUpdate {
        url: String,
        version: Option<String>,
        #[serde(default)]
        sha256: Option<String>,
    },
    CancelAppUpdate,
    RevealFile { path: PathBuf },
    OpenUrl { url: String },
    /// Withdraws this slot only.
    Dismiss { notice: String },
}

#[derive(Debug)]
pub enum ActionError {
    TooLarge(usize),
    Parse(serde_json::Error),
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge(len) => write!(f, "payload too large: {len} bytes"),
            Self::Parse(error) => write!(f, "invalid action payload: {error}"),
        }
    }
}

impl AppAction {
    pub fn to_target(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_target(text: &str) -> Result<Self, ActionError> {
        if text.len() > MAX_PAYLOAD_BYTES {
            return Err(ActionError::TooLarge(text.len()));
        }
        serde_json::from_str(text).map_err(ActionError::Parse)
    }
}

/// A queue control of the downloads manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueOp {
    Resume,
    Pause,
    Clear,
}

/// What [`route`] needs from the app. Calls come from the GTK thread; the work behind them runs off it.
pub trait ActionHost {
    fn withdraw(&self, notice: &str);
    fn open_entry(&self, kind: EntryKind, id: i64);
    fn open_downloads(&self, kind: EntryKind);
    fn open_item(&self, kind: EntryKind, entry_id: i64, item_id: i64);
    fn mark_read(&self, kind: EntryKind, entry_id: i64, urls: Vec<String>);
    fn download(&self, kind: EntryKind, entry_id: i64, urls: Vec<String>);
    fn queue(&self, kind: EntryKind, op: QueueOp);
    fn cancel_update(&self, kind: EntryKind);
    /// Stops the running restore (the same stop as the restore banner's Cancel).
    fn cancel_restore(&self);
    fn open_url(&self, url: &str);
    fn reveal(&self, path: &std::path::Path);
    /// The folders a reveal may name (design 6.1): the data folder, the downloads folder and the cache folder.
    fn reveal_roots(&self) -> Vec<PathBuf>;
    /// Starts the app-update download of `url` (already checked by [`update_url_ok`]).
    fn start_app_update(&self, url: &str, version: Option<String>, sha256: Option<String>);
    fn cancel_app_update(&self);
    /// An action no part of the app handles yet.
    fn unavailable(&self, what: &str);
}

fn valid_urls(urls: &[String]) -> bool {
    urls.len() <= MAX_URLS && urls.iter().all(|u| u.len() <= MAX_URL_BYTES)
}

fn web_url(url: &str) -> bool {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// Design 6.1: the asset url of an app update is `https`, on a GitHub release host, with no user info and the default port.
pub fn update_url_ok(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else { return false };
    parsed.scheme() == "https"
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.port().is_none()
        && matches!(parsed.host_str(), Some("github.com" | "objects.githubusercontent.com"))
}

/// Design 6.1: a path to reveal is absolute, has no `..`, and lies under one of `roots`.
pub fn reveal_path_ok(path: &std::path::Path, roots: &[PathBuf]) -> bool {
    path.is_absolute()
        && !path.components().any(|c| matches!(c, std::path::Component::ParentDir))
        && roots.iter().any(|root| path.starts_with(root))
}

/// Runs `action`. A rejected payload is logged and does nothing.
pub fn route(host: &dyn ActionHost, action: AppAction) {
    use AppAction::*;
    let withdraw = |notice: &Option<String>| {
        if let Some(id) = notice {
            host.withdraw(id);
        }
    };
    match action {
        OpenMangaDownloads => host.open_downloads(EntryKind::Manga),
        OpenAnimeDownloads => host.open_downloads(EntryKind::Anime),
        OpenChapter { manga_id, chapter_id } => host.open_item(EntryKind::Manga, manga_id, chapter_id),
        OpenEpisode { anime_id, episode_id } => host.open_item(EntryKind::Anime, anime_id, episode_id),
        OpenManga { manga_id: id, notice } | OpenAnime { anime_id: id, notice } if id <= 0 => {
            tracing::warn!(id, ?notice, "a notice action named an entry id that is not positive");
        }
        OpenManga { manga_id, notice } => {
            withdraw(&notice);
            host.open_entry(EntryKind::Manga, manga_id);
        }
        OpenAnime { anime_id, notice } => {
            withdraw(&notice);
            host.open_entry(EntryKind::Anime, anime_id);
        }
        MarkChaptersRead { manga_id: id, urls, notice }
        | MarkEpisodesSeen { anime_id: id, urls, notice }
        | DownloadChapters { manga_id: id, urls, notice }
        | DownloadEpisodes { anime_id: id, urls, notice }
            if id <= 0 || !valid_urls(&urls) =>
        {
            tracing::warn!(id, urls = urls.len(), ?notice, "a notice action carried an invalid entry id or url list");
        }
        MarkChaptersRead { manga_id, urls, notice } => {
            withdraw(&notice);
            host.mark_read(EntryKind::Manga, manga_id, urls);
        }
        MarkEpisodesSeen { anime_id, urls, notice } => {
            withdraw(&notice);
            host.mark_read(EntryKind::Anime, anime_id, urls);
        }
        DownloadChapters { manga_id, urls, notice } => {
            withdraw(&notice);
            host.download(EntryKind::Manga, manga_id, urls);
        }
        DownloadEpisodes { anime_id, urls, notice } => {
            withdraw(&notice);
            host.download(EntryKind::Anime, anime_id, urls);
        }
        ResumeMangaDownloads => host.queue(EntryKind::Manga, QueueOp::Resume),
        PauseMangaDownloads => host.queue(EntryKind::Manga, QueueOp::Pause),
        ClearMangaDownloads => host.queue(EntryKind::Manga, QueueOp::Clear),
        ResumeAnimeDownloads => host.queue(EntryKind::Anime, QueueOp::Resume),
        PauseAnimeDownloads => host.queue(EntryKind::Anime, QueueOp::Pause),
        ClearAnimeDownloads => host.queue(EntryKind::Anime, QueueOp::Clear),
        CancelLibraryUpdate { notice } => {
            withdraw(&notice);
            host.cancel_update(EntryKind::Manga);
        }
        CancelAnimeLibraryUpdate { notice } => {
            withdraw(&notice);
            host.cancel_update(EntryKind::Anime);
        }
        CancelRestore => {
            host.withdraw(kuro_backup::notify::ID_RESTORE_PROGRESS);
            host.cancel_restore();
        }
        StartAppUpdate { url, version, sha256 } if update_url_ok(&url) => host.start_app_update(&url, version, sha256),
        StartAppUpdate { url, .. } => tracing::warn!(%url, "a notice action named an update url outside the release hosts"),
        CancelAppUpdate => host.cancel_app_update(),
        RevealFile { path } if reveal_path_ok(&path, &host.reveal_roots()) => host.reveal(&path),
        RevealFile { path } => tracing::warn!(?path, "a notice action named a path that may not be revealed"),
        OpenUrl { url } if web_url(&url) => host.open_url(&url),
        OpenUrl { url } => tracing::warn!(%url, "a notice action named a url that is not http or https"),
        Dismiss { notice } => host.withdraw(&notice),
    }
}

/// The row check of an open-chapter or open-episode action (N9): navigates when both rows exist, otherwise shows the
/// `notify.error.item_missing` toast and does not navigate.
pub fn open_or_toast(found: bool, open: impl FnOnce(), toast: impl FnOnce(&str)) {
    if found {
        open();
    } else {
        toast(ITEM_MISSING_TOAST);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::path::Path;

    #[derive(Default)]
    struct Fake(RefCell<Vec<String>>);

    impl Fake {
        fn log(&self, event: String) {
            self.0.borrow_mut().push(event);
        }
        fn events(&self) -> Vec<String> {
            self.0.borrow().clone()
        }
    }

    impl ActionHost for Fake {
        fn withdraw(&self, notice: &str) {
            self.log(format!("withdraw {notice}"));
        }
        fn open_entry(&self, kind: EntryKind, id: i64) {
            self.log(format!("open_entry {kind:?} {id}"));
        }
        fn open_downloads(&self, kind: EntryKind) {
            self.log(format!("open_downloads {kind:?}"));
        }
        fn open_item(&self, kind: EntryKind, entry_id: i64, item_id: i64) {
            self.log(format!("open_item {kind:?} {entry_id} {item_id}"));
        }
        fn mark_read(&self, kind: EntryKind, entry_id: i64, urls: Vec<String>) {
            self.log(format!("mark_read {kind:?} {entry_id} {}", urls.len()));
        }
        fn download(&self, kind: EntryKind, entry_id: i64, urls: Vec<String>) {
            self.log(format!("download {kind:?} {entry_id} {}", urls.len()));
        }
        fn queue(&self, kind: EntryKind, op: QueueOp) {
            self.log(format!("queue {kind:?} {op:?}"));
        }
        fn cancel_update(&self, kind: EntryKind) {
            self.log(format!("cancel_update {kind:?}"));
        }
        fn open_url(&self, url: &str) {
            self.log(format!("open_url {url}"));
        }
        fn reveal(&self, path: &Path) {
            self.log(format!("reveal {}", path.display()));
        }
        fn cancel_restore(&self) {
            self.log("cancel_restore".to_owned());
        }
        fn unavailable(&self, what: &str) {
            self.log(format!("unavailable {what}"));
        }
        fn reveal_roots(&self) -> Vec<PathBuf> {
            vec!["/data".into(), "/downloads".into()]
        }
        fn start_app_update(&self, url: &str, version: Option<String>, sha256: Option<String>) {
            self.log(format!("start_app_update {url} {version:?} {sha256:?}"));
        }
        fn cancel_app_update(&self) {
            self.log("cancel_app_update".to_owned());
        }
    }

    fn run(action: AppAction) -> Vec<String> {
        let host = Fake::default();
        route(&host, action);
        host.events()
    }

    #[test]
    fn every_variant_survives_the_json_codec() {
        // T-35a-006
        let notice = Some("n".to_owned());
        let urls = vec!["/a".to_owned()];
        let all = vec![
            AppAction::OpenMangaDownloads,
            AppAction::OpenAnimeDownloads,
            AppAction::OpenChapter { manga_id: 1, chapter_id: 2 },
            AppAction::OpenEpisode { anime_id: 1, episode_id: 2 },
            AppAction::OpenManga { manga_id: 1, notice: notice.clone() },
            AppAction::OpenAnime { anime_id: 1, notice: None },
            AppAction::MarkChaptersRead { manga_id: 1, urls: urls.clone(), notice: notice.clone() },
            AppAction::MarkEpisodesSeen { anime_id: 1, urls: urls.clone(), notice: notice.clone() },
            AppAction::DownloadChapters { manga_id: 1, urls: urls.clone(), notice: notice.clone() },
            AppAction::DownloadEpisodes { anime_id: 1, urls, notice: notice.clone() },
            AppAction::ResumeMangaDownloads,
            AppAction::PauseMangaDownloads,
            AppAction::ClearMangaDownloads,
            AppAction::ResumeAnimeDownloads,
            AppAction::PauseAnimeDownloads,
            AppAction::ClearAnimeDownloads,
            AppAction::CancelLibraryUpdate { notice: notice.clone() },
            AppAction::CancelAnimeLibraryUpdate { notice: None },
            AppAction::CancelRestore,
            AppAction::StartAppUpdate { url: "https://x".into(), version: Some("1.0.0".into()), sha256: Some("ab".into()) },
            AppAction::CancelAppUpdate,
            AppAction::RevealFile { path: "/tmp/a".into() },
            AppAction::OpenUrl { url: "https://x".into() },
            AppAction::Dismiss { notice: "n".into() },
        ];
        for action in all {
            let text = action.to_target().unwrap();
            assert_eq!(AppAction::from_target(&text).unwrap(), action, "{text}");
        }
    }

    #[test]
    fn bad_json_and_oversized_payloads_are_errors() {
        assert!(matches!(AppAction::from_target("{nope"), Err(ActionError::Parse(_))));
        assert!(matches!(AppAction::from_target(r#"{"kind":"unknown"}"#), Err(ActionError::Parse(_))));
        let big = " ".repeat(MAX_PAYLOAD_BYTES + 1);
        assert!(matches!(AppAction::from_target(&big), Err(ActionError::TooLarge(_))));
    }

    #[test]
    fn dismiss_withdraws_and_does_nothing_else() {
        // T-35a-007
        assert_eq!(run(AppAction::Dismiss { notice: "n1".into() }), ["withdraw n1"]);
    }

    #[test]
    fn a_notice_is_withdrawn_before_the_work_starts() {
        // T-35a-007 order test
        let urls = vec!["/a".to_owned(), "/b".to_owned()];
        let with = |n: &str| Some(n.to_owned());
        assert_eq!(
            run(AppAction::MarkChaptersRead { manga_id: 4, urls: urls.clone(), notice: with("n") }),
            ["withdraw n", "mark_read Manga 4 2"]
        );
        assert_eq!(
            run(AppAction::MarkEpisodesSeen { anime_id: 4, urls: urls.clone(), notice: with("n") }),
            ["withdraw n", "mark_read Anime 4 2"]
        );
        assert_eq!(
            run(AppAction::DownloadChapters { manga_id: 4, urls: urls.clone(), notice: with("n") }),
            ["withdraw n", "download Manga 4 2"]
        );
        assert_eq!(
            run(AppAction::DownloadEpisodes { anime_id: 4, urls, notice: with("n") }),
            ["withdraw n", "download Anime 4 2"]
        );
        assert_eq!(run(AppAction::OpenManga { manga_id: 4, notice: with("n") }), ["withdraw n", "open_entry Manga 4"]);
        assert_eq!(run(AppAction::OpenAnime { anime_id: 4, notice: None }), ["open_entry Anime 4"]);
    }

    #[test]
    fn an_entry_id_that_is_not_positive_is_rejected_without_navigation() {
        // T-35a-015
        for id in [0, -3] {
            assert!(run(AppAction::OpenManga { manga_id: id, notice: Some("n".into()) }).is_empty());
            assert!(run(AppAction::OpenAnime { anime_id: id, notice: None }).is_empty());
            let mark = AppAction::MarkChaptersRead { manga_id: id, urls: vec![], notice: Some("n".into()) };
            assert!(run(mark).is_empty(), "no withdraw and no work either");
        }
    }

    #[test]
    fn untrusted_lists_and_urls_are_bounded() {
        let long = vec!["x".repeat(MAX_URL_BYTES + 1)];
        assert!(run(AppAction::DownloadChapters { manga_id: 1, urls: long, notice: None }).is_empty());
        let many = vec!["u".to_owned(); MAX_URLS + 1];
        assert!(run(AppAction::MarkChaptersRead { manga_id: 1, urls: many, notice: None }).is_empty());
        assert!(run(AppAction::OpenUrl { url: "file:///etc/passwd".into() }).is_empty());
        assert_eq!(run(AppAction::OpenUrl { url: "https://a.b".into() }), ["open_url https://a.b"]);
        assert!(run(AppAction::RevealFile { path: "rel/x".into() }).is_empty());
    }

    #[test]
    fn an_update_url_must_be_https_on_a_release_host() {
        // T-35a-092, test U13
        let start = |url: &str| run(AppAction::StartAppUpdate { url: url.into(), version: Some("v1".into()), sha256: None });
        let good = "https://github.com/o/r/releases/download/v1/App.AppImage";
        assert_eq!(start(good), [format!("start_app_update {good} Some(\"v1\") None")]);
        assert_eq!(start("https://objects.githubusercontent.com/x").len(), 1);
        for bad in [
            "http://github.com/o/r/a.AppImage",
            "https://evil.example/a.AppImage",
            "https://github.com.evil.example/a",
            "https://user@github.com/a",
            "https://github.com:8443/a",
            "file:///etc/passwd",
            "not a url",
        ] {
            assert!(start(bad).is_empty(), "{bad}");
        }
        assert_eq!(run(AppAction::CancelAppUpdate), ["cancel_app_update"]);
    }

    #[test]
    fn a_reveal_stays_inside_the_allowed_folders() {
        // T-35a-092
        let reveal = |p: &str| run(AppAction::RevealFile { path: p.into() });
        assert_eq!(reveal("/downloads/App.AppImage"), ["reveal /downloads/App.AppImage"]);
        assert_eq!(reveal("/data/x").len(), 1);
        assert!(reveal("/etc/passwd").is_empty());
        assert!(reveal("/downloads/../etc/passwd").is_empty());
        assert!(reveal("/downloadsX/a").is_empty());
    }

    #[test]
    fn the_six_queue_actions_reach_the_matching_manager() {
        // T-35a-020
        use QueueOp::*;
        let cases = [
            (AppAction::ResumeMangaDownloads, EntryKind::Manga, Resume),
            (AppAction::PauseMangaDownloads, EntryKind::Manga, Pause),
            (AppAction::ClearMangaDownloads, EntryKind::Manga, Clear),
            (AppAction::ResumeAnimeDownloads, EntryKind::Anime, Resume),
            (AppAction::PauseAnimeDownloads, EntryKind::Anime, Pause),
            (AppAction::ClearAnimeDownloads, EntryKind::Anime, Clear),
        ];
        for (action, kind, op) in cases {
            assert_eq!(run(action), [format!("queue {kind:?} {op:?}")]);
        }
    }

    #[test]
    fn the_cancel_actions_stop_the_matching_job_and_withdraw_their_notice() {
        // T-35a-021
        assert_eq!(
            run(AppAction::CancelLibraryUpdate { notice: Some("library.progress".into()) }),
            ["withdraw library.progress", "cancel_update Manga"]
        );
        assert_eq!(run(AppAction::CancelAnimeLibraryUpdate { notice: None }), ["cancel_update Anime"]);
    }

    #[test]
    fn cancel_restore_withdraws_the_restore_progress_and_then_stops_the_job() {
        // T-35a-022: the progress slot is withdrawn before the stop is asked for
        assert_eq!(run(AppAction::CancelRestore), ["withdraw restore-progress", "cancel_restore"]);
    }

    #[test]
    fn a_missing_row_toasts_and_does_not_navigate() {
        // T-35a-014, test N9
        let events = RefCell::new(Vec::new());
        open_or_toast(false, || events.borrow_mut().push("open".to_owned()), |t| events.borrow_mut().push(format!("toast {t}")));
        assert_eq!(*events.borrow(), ["toast Error"]);
        events.borrow_mut().clear();
        open_or_toast(true, || events.borrow_mut().push("open".to_owned()), |t| events.borrow_mut().push(format!("toast {t}")));
        assert_eq!(*events.borrow(), ["open"]);
        assert_eq!(run(AppAction::OpenChapter { manga_id: 1, chapter_id: 9 }), ["open_item Manga 1 9"]);
    }
}
