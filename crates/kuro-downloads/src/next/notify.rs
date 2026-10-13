//! The one download notifier, for both kinds (docs/design/unified-downloads.md 2.12). It replaces `notify::dispatch` and
//! `AnimeNotifier`. Per kind it has its own notification ids, its own error digest and its own "done" text. No progress
//! and no paused notice (user, 2026-10-12): only the end, warnings and failures are announced.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use kuro_platform::kind::Kind;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use super::events::DownloadEvent;
use crate::anime::ffmpeg::FFMPEG_HELP_URL;
use crate::downloader::BULK_WARNING_HELP_URL;
use crate::notify::{Action, DONE_TITLE, DOWNLOADER_TITLE, Notice, NotificationSink, Priority, UNKNOWN_ERROR_TEXT};

pub const MANGA_PROGRESS_ID: &str = "dl-manga-progress";
pub const MANGA_WARNING_ID: &str = "dl-manga-warning";
pub const MANGA_ERROR_ID: &str = "dl-manga-error";
pub const ANIME_PROGRESS_ID: &str = "dl-anime-progress";
pub const ANIME_WARNING_ID: &str = "dl-anime-warning";
pub const ANIME_ERROR_ID: &str = "dl-anime-error";

pub const MANGA_DONE_TEXT: &str = "Chapters are ready to read offline";
pub const ANIME_DONE_TEXT: &str = "Episodes are ready to watch offline";

/// Errors inside this window are summarised into one count (E-U02).
pub const ERROR_WINDOW: Duration = Duration::from_secs(5);

/// The notification ids of a kind: `(progress and done, warning, error)`.
fn ids(kind: Kind) -> (&'static str, &'static str, &'static str) {
    match kind {
        Kind::Manga => (MANGA_PROGRESS_ID, MANGA_WARNING_ID, MANGA_ERROR_ID),
        Kind::Anime => (ANIME_PROGRESS_ID, ANIME_WARNING_ID, ANIME_ERROR_ID),
    }
}

fn done_text(kind: Kind) -> &'static str {
    match kind {
        Kind::Manga => MANGA_DONE_TEXT,
        Kind::Anime => ANIME_DONE_TEXT,
    }
}

fn show_action(kind: Kind) -> Action {
    match kind {
        Kind::Manga => Action::ShowManga,
        Kind::Anime => Action::ShowAnime,
    }
}

/// The help link of a warning, when it is one this app knows (the queue-size warning or the ffmpeg help page).
pub fn known_help_url(url: Option<&str>) -> Option<&'static str> {
    [BULK_WARNING_HELP_URL, FFMPEG_HELP_URL].into_iter().find(|known| Some(*known) == url)
}

/// Counts errors in a window that starts at the first one. The first error in a window is shown on its own, each later
/// one in the same window turns the notice into a summary.
#[derive(Debug, Default)]
pub struct ErrorDigest {
    window_start: Option<Instant>,
    count: usize,
}

impl ErrorDigest {
    /// Records an error at `now`. `true` opens a new window (show the single notice), `false` joins one (show the
    /// summary of [`ErrorDigest::count`]).
    pub fn record(&mut self, now: Instant) -> bool {
        match self.window_start {
            Some(start) if now.saturating_duration_since(start) <= ERROR_WINDOW => {
                self.count += 1;
                false
            }
            _ => {
                self.window_start = Some(now);
                self.count = 1;
                true
            }
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }
}

pub struct DownloadNotifier {
    sink: Arc<dyn NotificationSink>,
    /// "Hide notification content": a failure notice then names neither the title nor the unit.
    hide_content: Arc<AtomicBool>,
    /// One digest per kind, in `Kind::ALL` order.
    errors: [ErrorDigest; 2],
}

impl DownloadNotifier {
    pub fn new(sink: Arc<dyn NotificationSink>, hide_content: Arc<AtomicBool>) -> Self {
        Self { sink, hide_content, errors: [ErrorDigest::default(), ErrorDigest::default()] }
    }

    /// Sends the notification an event needs, if any.
    pub fn dispatch(&mut self, now: Instant, event: &DownloadEvent) {
        match event {
            DownloadEvent::Complete { kind } => {
                let (progress, _, _) = ids(*kind);
                self.sink.show(&Notice {
                    id: progress,
                    priority: Priority::Low,
                    title: DONE_TITLE.into(),
                    body: Some(done_text(*kind).into()),
                    ongoing: false,
                    actions: Vec::new(),
                    expire_secs: None,
                });
            }
            DownloadEvent::Dismiss { kind } => self.sink.withdraw(ids(*kind).0),
            DownloadEvent::Warning { kind, text, timeout_secs, help_url } => {
                self.sink.show(&Notice {
                    id: ids(*kind).1,
                    priority: Priority::Low,
                    title: DOWNLOADER_TITLE.into(),
                    body: Some(text.clone()),
                    ongoing: false,
                    actions: known_help_url(help_url.as_deref()).map(Action::OpenHelp).into_iter().collect(),
                    expire_secs: *timeout_secs,
                });
            }
            DownloadEvent::Error { kind, entry_title, unit_name, message, .. } => {
                self.error(now, *kind, entry_title.as_deref(), unit_name.as_deref(), message);
            }
            // The app's own Downloads item shows progress and a paused queue.
            DownloadEvent::Progress { .. }
            | DownloadEvent::Paused { .. }
            | DownloadEvent::Started { .. }
            | DownloadEvent::Status { .. }
            | DownloadEvent::Finished { .. }
            | DownloadEvent::QueueChanged { .. } => {}
        }
    }

    fn error(&mut self, now: Instant, kind: Kind, entry_title: Option<&str>, unit_name: Option<&str>, message: &str) {
        let (_, _, error_id) = ids(kind);
        let digest = &mut self.errors[kind_slot(kind)];
        let notice = if digest.record(now) {
            let hide = self.hide_content.load(Ordering::Relaxed);
            let (entry_title, unit_name) = if hide { (None, None) } else { (entry_title, unit_name) };
            let title = match (entry_title, unit_name) {
                (Some(entry), Some(unit)) => format!("{entry}: {unit}"),
                (Some(entry), None) => entry.to_owned(),
                (None, _) => DOWNLOADER_TITLE.into(),
            };
            let body = if message.is_empty() { UNKNOWN_ERROR_TEXT.to_owned() } else { message.to_owned() };
            Notice {
                id: error_id,
                priority: Priority::Low,
                title,
                body: Some(body),
                ongoing: false,
                actions: if entry_title.is_some() { vec![show_action(kind)] } else { Vec::new() },
                expire_secs: None,
            }
        } else {
            let noun = match kind {
                Kind::Manga => "chapter",
                Kind::Anime => "episode",
            };
            Notice {
                id: error_id,
                priority: Priority::Low,
                title: DOWNLOADER_TITLE.into(),
                body: Some(format!("{} {noun} downloads failed", digest.count())),
                ongoing: false,
                actions: Vec::new(),
                expire_secs: None,
            }
        };
        self.sink.show(&notice);
    }
}

fn kind_slot(kind: Kind) -> usize {
    match kind {
        Kind::Manga => 0,
        Kind::Anime => 1,
    }
}

/// Forwards events to a [`DownloadNotifier`] until the channel closes. `hide_content` is read per event, so a settings
/// change applies to the next notice.
pub fn spawn_notifier(
    mut events: broadcast::Receiver<DownloadEvent>,
    sink: Arc<dyn NotificationSink>,
    hide_content: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut notifier = DownloadNotifier::new(sink, hide_content);
        loop {
            match events.recv().await {
                Ok(event) => notifier.dispatch(Instant::now(), &event),
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

    #[derive(Default)]
    struct Recorder {
        shown: Mutex<Vec<Notice>>,
        withdrawn: Mutex<Vec<&'static str>>,
    }

    impl NotificationSink for Recorder {
        fn show(&self, notice: &Notice) {
            self.shown.lock().unwrap().push(notice.clone());
        }
        fn withdraw(&self, id: &'static str) {
            self.withdrawn.lock().unwrap().push(id);
        }
    }

    fn notifier(hide: bool) -> (DownloadNotifier, Arc<Recorder>) {
        let rec = Arc::new(Recorder::default());
        let sink: Arc<dyn NotificationSink> = rec.clone();
        (DownloadNotifier::new(sink, Arc::new(AtomicBool::new(hide))), rec)
    }

    fn error(kind: Kind, entry: &str, unit: &str, message: &str) -> DownloadEvent {
        DownloadEvent::Error {
            kind,
            unit_id: Some(1),
            entry_id: Some(2),
            entry_title: Some(entry.into()),
            unit_name: Some(unit.into()),
            message: message.into(),
        }
    }

    #[test]
    fn the_end_says_so_per_kind_with_its_own_text_and_id() {
        let (mut n, rec) = notifier(false);
        let now = Instant::now();
        n.dispatch(now, &DownloadEvent::Complete { kind: Kind::Manga });
        n.dispatch(now, &DownloadEvent::Complete { kind: Kind::Anime });
        let shown = rec.shown.lock().unwrap();
        assert_eq!((shown[0].id, shown[0].body.as_deref()), (MANGA_PROGRESS_ID, Some(MANGA_DONE_TEXT)));
        assert_eq!((shown[1].id, shown[1].body.as_deref()), (ANIME_PROGRESS_ID, Some(ANIME_DONE_TEXT)));
        assert!(shown.iter().all(|x| x.title == DONE_TITLE && !x.ongoing && x.actions.is_empty()));
    }

    #[test]
    fn progress_pause_and_bookkeeping_show_nothing_and_a_dismiss_withdraws_its_kind() {
        let (mut n, rec) = notifier(false);
        let now = Instant::now();
        for kind in Kind::ALL {
            n.dispatch(now, &DownloadEvent::Paused { kind });
            n.dispatch(now, &DownloadEvent::Started { kind });
            n.dispatch(now, &DownloadEvent::QueueChanged { kind });
            n.dispatch(now, &DownloadEvent::Finished { kind, unit_id: 1, entry_id: 1 });
            n.dispatch(
                now,
                &DownloadEvent::Progress {
                    kind,
                    unit_id: 1,
                    entry_id: 1,
                    entry_title: "T".into(),
                    unit_name: "U".into(),
                    done: 1,
                    total: 2,
                    percent: 50,
                },
            );
        }
        assert!(rec.shown.lock().unwrap().is_empty());
        n.dispatch(now, &DownloadEvent::Dismiss { kind: Kind::Anime });
        assert_eq!(*rec.withdrawn.lock().unwrap(), vec![ANIME_PROGRESS_ID]);
    }

    #[test]
    fn a_warning_carries_its_timeout_and_offers_only_a_known_help_link() {
        let (mut n, rec) = notifier(false);
        let now = Instant::now();
        let warning = |kind, help_url: Option<&str>| DownloadEvent::Warning {
            kind,
            text: "big".into(),
            timeout_secs: Some(30),
            help_url: help_url.map(str::to_owned),
        };
        n.dispatch(now, &warning(Kind::Manga, Some(BULK_WARNING_HELP_URL)));
        n.dispatch(now, &warning(Kind::Anime, Some("https://example.org/unknown")));
        let shown = rec.shown.lock().unwrap();
        assert_eq!((shown[0].id, shown[0].title.as_str(), shown[0].expire_secs), (MANGA_WARNING_ID, DOWNLOADER_TITLE, Some(30)));
        assert_eq!(shown[0].actions, vec![Action::OpenHelp(BULK_WARNING_HELP_URL)]);
        assert_eq!(shown[1].id, ANIME_WARNING_ID);
        assert!(shown[1].actions.is_empty(), "an unknown link is not offered");
    }

    #[test]
    fn a_single_error_names_the_unit_and_a_second_in_the_window_summarises_per_kind() {
        let (mut n, rec) = notifier(false);
        let t0 = Instant::now();
        n.dispatch(t0, &error(Kind::Anime, "Show", "Episode 2", "HTTP error 500"));
        n.dispatch(t0 + Duration::from_secs(2), &error(Kind::Anime, "Show", "Episode 3", "timeout"));
        // The manga digest is separate: its first error is shown alone.
        n.dispatch(t0 + Duration::from_secs(2), &error(Kind::Manga, "Berserk", "Chapter 1", "timeout"));
        n.dispatch(t0 + Duration::from_secs(3), &error(Kind::Manga, "Berserk", "Chapter 2", "timeout"));
        let shown = rec.shown.lock().unwrap();
        assert_eq!(shown[0].title, "Show: Episode 2");
        assert_eq!(shown[0].body.as_deref(), Some("HTTP error 500"));
        assert_eq!(shown[0].actions, vec![Action::ShowAnime]);
        assert_eq!(shown[1].body.as_deref(), Some("2 episode downloads failed"));
        assert_eq!(shown[2].title, "Berserk: Chapter 1");
        assert_eq!(shown[2].actions, vec![Action::ShowManga]);
        assert_eq!(shown[3].body.as_deref(), Some("2 chapter downloads failed"));
        assert!(shown[..2].iter().all(|x| x.id == ANIME_ERROR_ID) && shown[2..].iter().all(|x| x.id == MANGA_ERROR_ID));
    }

    #[test]
    fn an_error_after_the_window_starts_a_new_single_notice() {
        let mut digest = ErrorDigest::default();
        let t0 = Instant::now();
        assert!(digest.record(t0));
        assert!(!digest.record(t0 + Duration::from_secs(1)));
        assert_eq!(digest.count(), 2);
        assert!(digest.record(t0 + Duration::from_secs(9)));
        assert_eq!(digest.count(), 1);
    }

    #[test]
    fn an_error_without_a_title_or_message_falls_back_to_the_plain_text() {
        let (mut n, rec) = notifier(false);
        let bare = DownloadEvent::Error {
            kind: Kind::Manga,
            unit_id: None,
            entry_id: None,
            entry_title: None,
            unit_name: None,
            message: String::new(),
        };
        n.dispatch(Instant::now(), &bare);
        let shown = rec.shown.lock().unwrap();
        assert_eq!(shown[0].title, DOWNLOADER_TITLE);
        assert_eq!(shown[0].body.as_deref(), Some(UNKNOWN_ERROR_TEXT));
        assert!(shown[0].actions.is_empty() && !shown[0].title.contains("null"));
    }

    #[test]
    fn hidden_content_keeps_the_title_and_the_unit_out_of_a_failure_notice() {
        let (mut n, rec) = notifier(true);
        n.dispatch(Instant::now(), &error(Kind::Manga, "Berserk", "Chapter 1", "HTTP error 500"));
        let shown = rec.shown.lock().unwrap();
        assert_eq!(shown[0].title, DOWNLOADER_TITLE);
        assert_eq!(shown[0].body.as_deref(), Some("HTTP error 500"));
        assert!(shown[0].actions.is_empty());
    }

    #[test]
    fn every_notice_is_low_importance() {
        let (mut n, rec) = notifier(false);
        let now = Instant::now();
        n.dispatch(now, &DownloadEvent::Complete { kind: Kind::Manga });
        n.dispatch(now, &DownloadEvent::Warning { kind: Kind::Anime, text: "w".into(), timeout_secs: None, help_url: None });
        n.dispatch(now, &error(Kind::Anime, "S", "E", "e"));
        assert!(rec.shown.lock().unwrap().iter().all(|x| x.priority == Priority::Low));
    }
}
