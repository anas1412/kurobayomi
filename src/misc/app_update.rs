//! The desktop notices of the app update (T-35a-048, T-35a-051, T-35a-052, T-35a-053): the prompt for a new release, the
//! download progress, the failure with its retry, and the finished download. Each notice is built here as a plain
//! [`Post`]; the ordered steps a download runs (withdraw first, then show) are [`Op`] lists, so the order is testable
//! without GTK.
//!
//! Decisions:
//! - The prompt, the progress and the download share the slot `app-update.progress` (E-U4 keeps the one slot), so a
//!   started download replaces the prompt rather than stacking under it.
//! - The title of the progress and the prompt is the version tag. A download started without a version (not reachable
//!   from the prompt or Retry) shows the fallback title [`DOWNLOAD_TITLE`].
//! - "Open folder" reveals the downloaded file itself: the file manager opens its folder with the file selected, so it
//!   is the same action as the default tap.
//! - The file is only revealed, never executed (T-35a-053).

use std::path::Path;

use super::ids;
use super::notify::{Op, Post, Target};
use super::router::AppAction;

/// The title of a download with no known version.
pub const DOWNLOAD_TITLE: &str = "Downloading update";
/// The text of the failure notice title.
pub const FAILED_TITLE: &str = "Update download failed";
/// The text of the finished notice title.
pub const DONE_TITLE: &str = "Update downloaded";

/// What the progress notice says in its body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Downloading; `Some(percent)` when the length is known (T-35a-051 test N8 shows no percent otherwise).
    Downloading(Option<u8>),
    /// No connection: no request is sent until one comes back (T-35a-057).
    WaitingForNetwork,
}



/// The progress notice: low priority, one "Cancel" button. The caller throttles the percents (`ProgressThrottle`).
pub fn progress(version: Option<&str>, phase: Phase) -> Post {
    let body = match phase {
        Phase::Downloading(Some(percent)) => format!("Downloading {percent}%"),
        Phase::Downloading(None) => "Downloading\u{2026}".to_owned(),
        Phase::WaitingForNetwork => "Waiting for network".to_owned(),
    };
    Post {
        id: ids::APP_UPDATE_PROGRESS.to_owned(),
        title: version.map_or_else(|| DOWNLOAD_TITLE.to_owned(), str::to_owned),
        body: Some(body),
        low_priority: true,
        timeout: None,
        tap: None,
        buttons: vec![("Cancel".to_owned(), Target::Action(AppAction::CancelAppUpdate))],
    }
}

/// The failure notice: "Retry" starts the same download again with its version, "Cancel" dismisses this slot only.
pub fn failure(version: Option<&str>, url: &str, sha256: Option<&str>, message: &str) -> Post {
    Post {
        id: ids::APP_UPDATE_ERROR.to_owned(),
        title: FAILED_TITLE.to_owned(),
        body: Some(message.to_owned()),
        low_priority: false,
        timeout: None,
        tap: None,
        buttons: vec![
            (
                "Retry".to_owned(),
                Target::Action(AppAction::StartAppUpdate { url: url.to_owned(), version: version.map(str::to_owned), sha256: sha256.map(str::to_owned) }),
            ),
            ("Cancel".to_owned(), Target::Action(AppAction::Dismiss { notice: ids::APP_UPDATE_ERROR.to_owned() })),
        ],
    }
}

/// The finished notice for the downloaded `path`. Without a checksum the body says "(not verified)".
pub fn finished(path: &Path, verified: bool) -> Post {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let suffix = if verified { "" } else { " (not verified)" };
    let reveal = Target::Action(AppAction::RevealFile { path: path.to_path_buf() });
    Post {
        id: ids::APP_UPDATE_INSTALL.to_owned(),
        title: DONE_TITLE.to_owned(),
        body: Some(format!("Downloaded {name}{suffix}")),
        low_priority: false,
        timeout: None,
        tap: Some(reveal.clone()),
        buttons: vec![
            ("Open folder".to_owned(), reveal),
            ("Dismiss".to_owned(), Target::Action(AppAction::Dismiss { notice: ids::APP_UPDATE_INSTALL.to_owned() })),
        ],
    }
}

/// The steps of a failed download: the progress is withdrawn before the error is shown (T-35a-052).
pub fn failed_steps(version: Option<&str>, url: &str, sha256: Option<&str>, message: &str) -> Vec<Op> {
    vec![Op::Withdraw(ids::APP_UPDATE_PROGRESS.to_owned()), Op::Show(failure(version, url, sha256, message))]
}

/// The steps of a cancelled download: the progress goes, and no error is shown (T-35a-052, test U17).
pub fn cancelled_steps() -> Vec<Op> {
    vec![Op::Withdraw(ids::APP_UPDATE_PROGRESS.to_owned())]
}

/// The steps of a finished download: the progress goes, then the finished notice (T-35a-053).
pub fn finished_steps(path: &Path, verified: bool) -> Vec<Op> {
    vec![Op::Withdraw(ids::APP_UPDATE_PROGRESS.to_owned()), Op::Show(finished(path, verified))]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const ASSET: &str = "https://github.com/owner/repo/releases/download/v1.2.0/Kurobayomi.AppImage";

    /// The order of steps as a log: what a fake notifier would record.
    fn log(ops: &[Op]) -> Vec<String> {
        ops.iter()
            .map(|op| match op {
                Op::Withdraw(id) => format!("withdraw:{id}"),
                Op::Show(post) => format!("show:{}", post.id),
            })
            .collect()
    }

    #[test]
    fn the_progress_shows_the_version_and_the_percent_and_a_cancel_button() {
        // T-35a-051: "Downloading…" then "Downloading 42%"; Cancel => CancelAppUpdate
        let early = progress(Some("1.2.0"), Phase::Downloading(None));
        assert_eq!(early.body.as_deref(), Some("Downloading\u{2026}"));
        assert_eq!(early.title, "1.2.0");
        assert!(early.low_priority);
        assert_eq!(early.buttons, vec![("Cancel".to_owned(), Target::Action(AppAction::CancelAppUpdate))]);

        let later = progress(Some("1.2.0"), Phase::Downloading(Some(42)));
        assert_eq!(later.body.as_deref(), Some("Downloading 42%"));
    }

    #[test]
    fn an_unknown_length_never_shows_a_percent() {
        // N8: no percentage with an unknown length
        let post = progress(Some("1.2.0"), Phase::Downloading(None));
        assert!(!post.body.unwrap_or_default().contains('%'));
    }

    #[test]
    fn a_retried_download_keeps_the_version_in_the_title() {
        // N6 (E-U3): the retry carries the version, so the progress title is the version again
        let steps = failed_steps(Some("1.2.0"), ASSET, None, "network error");
        let Op::Show(error) = &steps[1] else { panic!("the second step is the error") };
        assert_eq!(error.buttons[0].1, Target::Action(AppAction::StartAppUpdate { url: ASSET.into(), version: Some("1.2.0".into()), sha256: None }));
        let retried = progress(Some("1.2.0"), Phase::Downloading(None));
        assert_eq!(retried.title, "1.2.0");
    }

    #[test]
    fn a_failed_download_withdraws_the_progress_before_the_error_is_shown() {
        // N5 (T-35a-052): the order is part of the contract
        let steps = failed_steps(Some("1.2.0"), ASSET, None, "HTTP 404");
        assert_eq!(log(&steps), vec!["withdraw:app-update.progress", "show:app-update.error"]);
    }

    #[test]
    fn the_error_has_retry_and_cancel_and_cancel_dismisses_only_its_slot() {
        let post = failure(Some("1.2.0"), ASSET, None, "HTTP 404");
        assert_eq!(post.id, "app-update.error");
        assert_eq!(post.buttons.len(), 2);
        assert_eq!(post.buttons[1], ("Cancel".to_owned(), Target::Action(AppAction::Dismiss { notice: "app-update.error".into() })));
    }

    #[test]
    fn a_cancelled_download_withdraws_the_progress_and_shows_no_error() {
        // U17: the cancel path ends with no error notice
        assert_eq!(log(&cancelled_steps()), vec!["withdraw:app-update.progress"]);
    }

    #[test]
    fn a_finished_download_reveals_the_file_and_never_opens_it() {
        // T-35a-053: the default action and "Open folder" reveal the file; no step runs it
        let path = PathBuf::from("/home/me/Downloads/Kurobayomi.AppImage");
        let steps = finished_steps(&path, true);
        assert_eq!(log(&steps), vec!["withdraw:app-update.progress", "show:app-update.install"]);
        let Op::Show(post) = &steps[1] else { panic!("the second step is the notice") };
        assert_eq!(post.body.as_deref(), Some("Downloaded Kurobayomi.AppImage"));
        assert_eq!(post.tap, Some(Target::Action(AppAction::RevealFile { path: path.clone() })));
        assert_eq!(post.buttons[0].0, "Open folder");
        assert_eq!(post.buttons[1], ("Dismiss".to_owned(), Target::Action(AppAction::Dismiss { notice: "app-update.install".into() })));
        for (_, target) in &post.buttons {
            assert!(matches!(target, Target::Action(AppAction::RevealFile { .. } | AppAction::Dismiss { .. })));
        }
    }

    #[test]
    fn a_download_without_a_checksum_says_it_is_not_verified() {
        let post = finished(Path::new("/d/app.AppImage"), false);
        assert_eq!(post.body.as_deref(), Some("Downloaded app.AppImage (not verified)"));
    }

    #[test]
    fn the_waiting_phase_says_so_and_shows_no_percent() {
        // T-35a-057 text
        let post = progress(None, Phase::WaitingForNetwork);
        assert_eq!(post.body.as_deref(), Some("Waiting for network"));
        assert_eq!(post.title, DOWNLOAD_TITLE);
    }

}
