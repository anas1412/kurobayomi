//! Downloader notifications: progress, paused, complete, warning and error (port of MangaDownloadNotifier.kt and the
//! title rules of Ntf.kt). The text rules are pure functions. [`spawn_notifier`] turns downloader events into calls on
//! a [`NotificationSink`], which the GUI implements over `gio::Notification`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use kuro_core::text::chop;
use kuro_platform::notifier::{APP_NAME, Notifier, send};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::events::DownloadEvent;

/// Replaces the progress notification on every update, and after pause or completion (pitfall 15).
pub const PROGRESS_ID: &str = "dl-progress";
pub const WARNING_ID: &str = "dl-warning";
pub const ERROR_ID: &str = "dl-error";

pub const DOWNLOADER_TITLE: &str = "Downloader";
pub const PAUSED_TITLE: &str = "Paused";
pub const PAUSED_TEXT: &str = "Chapter download paused";
pub const UNKNOWN_ERROR_TEXT: &str = "Could not download chapter due to unexpected error";
const MANGA_TITLE_CAP: usize = 15;
const CONTENT_TITLE_CAP: usize = 30;
const ELLIPSIS: &str = "…";

/// Notification importance class. The downloader only uses `Low`: no sound, for every kind (T-23a-019, D-DL-03).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Low,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub id: &'static str,
    pub priority: Priority,
    pub title: String,
    pub body: Option<String>,
    pub ongoing: bool,
    pub actions: Vec<Action>,
    /// Seconds before the notice expires on its own (`setTimeoutAfter`).
    pub expire_secs: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Pause,
    Resume,
    CancelAll,
    ShowManga,
    ShowAnime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Show(Notice),
    Withdraw(&'static str),
}

/// Where notifications go. The GUI implements it over `gio::Notification`, replacing by id.
pub trait NotificationSink: Send + Sync {
    fn show(&self, notice: &Notice);
    fn withdraw(&self, id: &'static str);
}

/// The body of a download notice while `hide_notification_content` is on.
pub const GENERIC_TEXT: &str = "Downloads in progress";

/// A [`NotificationSink`] over the platform [`Notifier`]. Every download notice is sensitive (T-01a-034): with
/// `hide_content` set, the title is the app name and the body is [`GENERIC_TEXT`]. The flag is read per notice.
pub struct NotifierSink {
    notifier: Arc<dyn Notifier>,
    hide_content: Arc<AtomicBool>,
}

impl NotifierSink {
    pub fn new(notifier: Arc<dyn Notifier>, hide_content: Arc<AtomicBool>) -> Self {
        Self { notifier, hide_content }
    }
}

impl NotificationSink for NotifierSink {
    fn show(&self, notice: &Notice) {
        let body = notice.body.as_deref().unwrap_or_default();
        let hide = self.hide_content.load(Ordering::Relaxed);
        send(self.notifier.as_ref(), hide, true, notice.id, APP_NAME, &notice.title, body, GENERIC_TEXT);
    }

    fn withdraw(&self, id: &'static str) {
        self.notifier.withdraw(id);
    }
}

/// Pure mapping from an event to the notification it needs, if any.
pub fn effect_for(event: &DownloadEvent, hide_content: bool) -> Option<Effect> {
    match event {
        DownloadEvent::Progress { manga_title, chapter_name, downloaded, total, .. } => {
            let progress = format!("Downloading ({downloaded}/{total})");
            let (title, body) = if hide_content {
                (progress, None)
            } else {
                (progress_title(manga_title, chapter_name), Some(progress))
            };
            Some(Effect::Show(Notice {
                id: PROGRESS_ID,
                priority: Priority::Low,
                title,
                body,
                ongoing: true,
                actions: vec![Action::Pause, Action::ShowManga],
                expire_secs: None,
            }))
        }
        DownloadEvent::Paused => Some(Effect::Show(Notice {
            id: PROGRESS_ID,
            priority: Priority::Low,
            title: PAUSED_TITLE.into(),
            body: Some(PAUSED_TEXT.into()),
            ongoing: false,
            actions: vec![Action::Resume, Action::CancelAll],
            expire_secs: None,
        })),
        DownloadEvent::Complete | DownloadEvent::Dismiss => Some(Effect::Withdraw(PROGRESS_ID)),
        DownloadEvent::Warning { text, timeout_secs, .. } => Some(Effect::Show(Notice {
            id: WARNING_ID,
            priority: Priority::Low,
            title: DOWNLOADER_TITLE.into(),
            body: Some(text.clone()),
            ongoing: false,
            actions: Vec::new(),
            expire_secs: *timeout_secs,
        })),
        DownloadEvent::Error { manga_title, chapter_name, message, .. } => {
            let title = match (manga_title, chapter_name) {
                (Some(manga), Some(chapter)) => format!("{manga}: {chapter}"),
                (Some(manga), None) => manga.clone(),
                (None, _) => DOWNLOADER_TITLE.into(),
            };
            let body = if message.is_empty() { UNKNOWN_ERROR_TEXT.into() } else { message.clone() };
            Some(Effect::Show(Notice {
                id: ERROR_ID,
                priority: Priority::Low,
                title,
                body: Some(body),
                ongoing: false,
                actions: if manga_title.is_some() { vec![Action::ShowManga] } else { Vec::new() },
                expire_secs: None,
            }))
        }
        DownloadEvent::Started
        | DownloadEvent::Status { .. }
        | DownloadEvent::Finished { .. }
        | DownloadEvent::QueueChanged => None,
    }
}

/// Title of the progress notification: the manga title (15 units at most) and the chapter name without its manga
/// prefix, at most 30 units. The prefix is removed before the title is cut, so long titles strip correctly (EC-108).
pub fn progress_title(manga_title: &str, chapter_name: &str) -> String {
    let chapter = strip_title_prefix(chapter_name, manga_title);
    let manga = chop(manga_title, MANGA_TITLE_CAP, ELLIPSIS).unwrap_or_else(|_| manga_title.to_owned());
    chop(&format!("{manga} - {chapter}"), CONTENT_TITLE_CAP, ELLIPSIS).unwrap_or_else(|_| format!("{manga} - {chapter}"))
}

/// Removes a leading manga title (case-insensitive), then whitespace, dashes and whitespace, as Kotlin's
/// `Regex("$title[\\s]*[-]*[\\s]*", IGNORE_CASE)` does.
fn strip_title_prefix<'a>(chapter: &'a str, manga_title: &str) -> &'a str {
    if manga_title.is_empty() {
        return chapter;
    }
    let Some(rest) = strip_prefix_ignore_case(chapter, manga_title) else {
        return chapter;
    };
    let rest = rest.trim_start_matches(char::is_whitespace);
    let rest = rest.trim_start_matches('-');
    rest.trim_start_matches(char::is_whitespace)
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let mut rest = text;
    for want in prefix.chars() {
        let got = rest.chars().next()?;
        if !got.to_lowercase().eq(want.to_lowercase()) {
            return None;
        }
        rest = &rest[got.len_utf8()..];
    }
    Some(rest)
}

/// Sends the effect of one event to the sink.
pub fn dispatch(event: &DownloadEvent, hide_content: bool, sink: &dyn NotificationSink) {
    match effect_for(event, hide_content) {
        Some(Effect::Show(notice)) => sink.show(&notice),
        Some(Effect::Withdraw(id)) => sink.withdraw(id),
        None => {}
    }
}

/// Forwards events to `sink` until the channel closes. `hide_content` is read per event, so a settings change
/// applies to the next update (FM-08).
pub fn spawn_notifier(
    mut events: broadcast::Receiver<DownloadEvent>,
    sink: Arc<dyn NotificationSink>,
    hide_content: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => dispatch(&event, hide_content.load(Ordering::Relaxed), sink.as_ref()),
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn progress(manga: &str, chapter: &str, done: usize, total: usize) -> DownloadEvent {
        DownloadEvent::Progress {
            chapter_id: 1,
            manga_title: manga.into(),
            chapter_name: chapter.into(),
            downloaded: done,
            total,
        }
    }

    fn shown(event: &DownloadEvent, hide: bool) -> Notice {
        match effect_for(event, hide) {
            Some(Effect::Show(n)) => n,
            other => panic!("expected a notice, got {other:?}"),
        }
    }

    #[test]
    fn progress_title_rules_ec_107_to_ec_109() {
        // EC-109: the manga title is removed from the chapter name, ignoring case and separators.
        assert_eq!(progress_title("One Piece", "one piece - Chapter 5"), "One Piece - Chapter 5");
        // Plain names are kept.
        assert_eq!(progress_title("Berserk", "Chapter 1"), "Berserk - Chapter 1");
        // The content title is cut to 30 units with an ellipsis.
        let long = progress_title("Berserk", &"x".repeat(100));
        assert_eq!(long.encode_utf16().count(), 30);
        assert!(long.ends_with('…'));
        // EC-108: a 30-unit manga title is cut to 15 units before the prefix rule applies.
        let t = progress_title("A very long manga title here", "A very long manga title here - Chapter 3");
        assert_eq!(t, "A very long ma… - Chapter 3");
    }

    #[test]
    fn hidden_content_shows_only_the_counts() {
        let n = shown(&progress("Berserk", "Chapter 1", 3, 10), true);
        assert_eq!(n.title, "Downloading (3/10)");
        assert_eq!(n.body, None);
        assert_eq!(n.id, PROGRESS_ID);
        let open = shown(&progress("Berserk", "Chapter 1", 3, 10), false);
        assert_eq!(open.body.as_deref(), Some("Downloading (3/10)"));
        assert!(open.ongoing);
        assert_eq!(open.actions, vec![Action::Pause, Action::ShowManga]);
    }

    #[test]
    fn paused_replaces_progress_with_resume_and_cancel() {
        let n = shown(&DownloadEvent::Paused, false);
        assert_eq!(n.id, PROGRESS_ID);
        assert_eq!(n.title, "Paused");
        assert_eq!(n.body.as_deref(), Some("Chapter download paused"));
        assert_eq!(n.actions, vec![Action::Resume, Action::CancelAll]);
        assert!(!n.ongoing);
    }

    #[test]
    fn complete_and_dismiss_withdraw_the_progress_notice() {
        assert_eq!(effect_for(&DownloadEvent::Complete, false), Some(Effect::Withdraw(PROGRESS_ID)));
        assert_eq!(effect_for(&DownloadEvent::Dismiss, false), Some(Effect::Withdraw(PROGRESS_ID)));
    }

    #[test]
    fn error_title_without_chapter_has_no_null_text_ec_110() {
        let with_chapter = DownloadEvent::Error {
            chapter_id: Some(1),
            manga_id: Some(2),
            manga_title: Some("Berserk".into()),
            chapter_name: Some("Chapter 1".into()),
            message: "HTTP error 500".into(),
        };
        let n = shown(&with_chapter, false);
        assert_eq!(n.title, "Berserk: Chapter 1");
        assert_eq!(n.body.as_deref(), Some("HTTP error 500"));
        assert_eq!(n.actions, vec![Action::ShowManga]);

        let without = DownloadEvent::Error {
            chapter_id: None,
            manga_id: Some(2),
            manga_title: Some("Berserk".into()),
            chapter_name: None,
            message: String::new(),
        };
        let n = shown(&without, false);
        assert_eq!(n.title, "Berserk");
        assert_eq!(n.body.as_deref(), Some(UNKNOWN_ERROR_TEXT), "TST-52 fallback text");
        assert!(!n.title.contains("null"));
        assert!(!n.ongoing);
    }

    #[test]
    fn warning_carries_its_timeout_and_low_priority_id() {
        let w = DownloadEvent::Warning { text: "big".into(), timeout_secs: Some(30), help_url: None };
        let n = shown(&w, false);
        assert_eq!(n.id, WARNING_ID);
        assert_eq!(n.expire_secs, Some(30));
        assert_eq!(n.title, DOWNLOADER_TITLE);
    }

    #[test]
    fn every_downloader_notice_is_low_importance() {
        let events = [
            progress("M", "C", 1, 2),
            DownloadEvent::Paused,
            DownloadEvent::Warning { text: "w".into(), timeout_secs: None, help_url: None },
            DownloadEvent::Error { chapter_id: None, manga_id: None, manga_title: None, chapter_name: None, message: "e".into() },
        ];
        for event in &events {
            assert_eq!(shown(event, false).priority, Priority::Low);
        }
    }

    #[test]
    fn status_and_bookkeeping_events_send_nothing() {
        assert_eq!(effect_for(&DownloadEvent::Started, false), None);
        assert_eq!(effect_for(&DownloadEvent::QueueChanged, false), None);
        assert_eq!(effect_for(&DownloadEvent::Finished { chapter_id: 1 }, false), None);
    }

    #[derive(Default)]
    struct FakeNotifier(Mutex<Vec<(String, String, Option<String>)>>);

    impl Notifier for FakeNotifier {
        fn show(&self, id: &str, title: &str, body: Option<&str>) {
            self.0.lock().unwrap().push((id.into(), title.into(), body.map(str::to_owned)));
        }
        fn withdraw(&self, _id: &str) {}
    }

    #[test]
    fn the_platform_sink_marks_download_notices_sensitive() {
        let fake = Arc::new(FakeNotifier::default());
        let hide = Arc::new(AtomicBool::new(false));
        let sink = NotifierSink::new(fake.clone(), hide.clone());
        let notice = shown(&progress("Berserk", "Chapter 1", 3, 10), false);
        sink.show(&notice);
        hide.store(true, Ordering::Relaxed);
        sink.show(&notice);
        let calls = fake.0.lock().unwrap();
        assert_eq!(calls[0], (PROGRESS_ID.into(), notice.title.clone(), notice.body.clone()));
        assert_eq!(calls[1], (PROGRESS_ID.into(), APP_NAME.into(), Some(GENERIC_TEXT.into())));
    }

    #[derive(Default)]
    struct Recorder(Mutex<Vec<String>>);

    impl NotificationSink for Recorder {
        fn show(&self, notice: &Notice) {
            self.0.lock().unwrap().push(format!("show {}", notice.id));
        }
        fn withdraw(&self, id: &'static str) {
            self.0.lock().unwrap().push(format!("withdraw {id}"));
        }
    }

    #[test]
    fn one_progress_id_across_progress_pause_and_complete() {
        let sink = Recorder::default();
        dispatch(&progress("M", "C", 1, 2), false, &sink);
        dispatch(&DownloadEvent::Paused, false, &sink);
        dispatch(&DownloadEvent::Complete, false, &sink);
        assert_eq!(
            *sink.0.lock().unwrap(),
            vec![format!("show {PROGRESS_ID}"), format!("show {PROGRESS_ID}"), format!("withdraw {PROGRESS_ID}")]
        );
    }
}
