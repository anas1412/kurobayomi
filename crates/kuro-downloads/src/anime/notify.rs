//! Anime download notifications (T-24a-026, T-24a-027, T-24a-028, T-24a-029; behaviour 10.2 to 10.6).
//!
//! One progress slot, one error slot. Progress is refreshed at most once per second, and at once on a state change.
//! Warnings and errors each build a fresh [`Notice`], so no state leaks from one into the next (E-U03). Errors that
//! arrive within five seconds are summarised into one count (E-U02). The title rules are the manga ones in
//! [`crate::notify::progress_title`], which strip the episode prefix before the title is cut (E-U01).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::anime::ffmpeg::FFMPEG_HELP_URL;
use crate::downloader::BULK_WARNING_HELP_URL;
use crate::events::DownloadEvent;
use crate::notify::{Action, NotificationSink, Notice, Priority, DOWNLOADER_TITLE, UNKNOWN_ERROR_TEXT, progress_title};

/// Progress slot (`ID_DOWNLOAD_EPISODE_PROGRESS`, -203). Kept apart from the manga progress id.
pub const PROGRESS_ID: &str = "dl-anime-progress";
/// Error slot (`ID_DOWNLOAD_EPISODE_ERROR`, -204). Warnings share it, as on Android.
pub const ERROR_ID: &str = "dl-anime-error";
/// Shortest gap between two progress updates (proposal, behaviour 10.2).
pub const PROGRESS_INTERVAL: Duration = Duration::from_secs(1);
/// Warning timeout (`AnimeDownloader.kt:941`).
pub const WARNING_TIMEOUT_SECS: u32 = 30;
/// Errors inside this window are summarised (proposal, E-U02).
pub const ERROR_WINDOW: Duration = Duration::from_secs(5);
pub const DONE_TEXT: &str = "Episodes are ready to watch offline";
pub const DOWNLOADING_TEXT: &str = "Downloading…";

/// The text of the progress line: `Downloading…` at 0, else `Downloading (N%)`.
pub fn progress_text(percent: i32) -> String {
    if percent <= 0 { DOWNLOADING_TEXT.to_owned() } else { format!("Downloading ({percent}%)") }
}

/// The progress notice. With `hide_content` the title is the progress text and the body is empty (T-24a-028).
pub fn progress_notice(anime_title: &str, episode_name: &str, percent: i32, hide_content: bool) -> Notice {
    let text = progress_text(percent);
    let (title, body) = if hide_content {
        (text, None)
    } else {
        (progress_title(anime_title, episode_name), Some(text))
    };
    Notice {
        id: PROGRESS_ID,
        priority: Priority::Low,
        title,
        body,
        ongoing: true,
        actions: vec![Action::Stop, Action::ShowAnime],
        expire_secs: None,
    }
}

/// The notice of a finished queue: "Downloads finished", not ongoing, no actions.
pub fn done_notice() -> Notice {
    Notice {
        id: PROGRESS_ID,
        priority: Priority::Low,
        title: crate::notify::DONE_TITLE.into(),
        body: Some(DONE_TEXT.into()),
        ongoing: false,
        actions: Vec::new(),
        expire_secs: None,
    }
}

/// The warning notice: title "Downloader", the reason as the body, auto-expiring after 30 s. With a help link it offers
/// the help action (T-24a-065).
pub fn warning_notice(text: &str, help_url: Option<&'static str>) -> Notice {
    Notice {
        id: ERROR_ID,
        priority: Priority::Low,
        title: DOWNLOADER_TITLE.into(),
        body: Some(text.to_owned()),
        ongoing: false,
        actions: help_url.map(Action::OpenHelp).into_iter().collect(),
        expire_secs: Some(WARNING_TIMEOUT_SECS),
    }
}

/// The help link of a warning, when it is one this app knows (the queue-size warning or the ffmpeg help page).
pub fn known_help_url(url: Option<&str>) -> Option<&'static str> {
    [BULK_WARNING_HELP_URL, FFMPEG_HELP_URL].into_iter().find(|known| Some(*known) == url)
}

/// The error notice for one episode: `"<anime>: <episode>"`, or the plain anime title, or "Downloader" (behaviour 10.6).
pub fn error_notice(anime_title: Option<&str>, episode: Option<&str>, message: &str) -> Notice {
    let title = match (anime_title, episode) {
        (Some(anime), Some(episode)) => format!("{anime}: {episode}"),
        (Some(anime), None) => anime.to_owned(),
        (None, _) => DOWNLOADER_TITLE.into(),
    };
    let body = if message.is_empty() { UNKNOWN_ERROR_TEXT.to_owned() } else { message.to_owned() };
    Notice {
        id: ERROR_ID,
        priority: Priority::Low,
        title,
        body: Some(body),
        ongoing: false,
        actions: if anime_title.is_some() { vec![Action::ShowAnime] } else { Vec::new() },
        expire_secs: None,
    }
}

/// The summary that replaces the single error when a second one arrives inside [`ERROR_WINDOW`].
pub fn error_summary_notice(count: usize) -> Notice {
    Notice {
        id: ERROR_ID,
        priority: Priority::Low,
        title: DOWNLOADER_TITLE.into(),
        body: Some(format!("{count} episode downloads failed")),
        ongoing: false,
        actions: Vec::new(),
        expire_secs: None,
    }
}

/// Counts errors in a window that starts at the first one. The first error in a window is shown on its own; each
/// later one in the same window turns the notice into a summary.
#[derive(Debug, Default)]
pub struct ErrorDigest {
    window_start: Option<Instant>,
    count: usize,
}

impl ErrorDigest {
    /// Records an error at `now`. Returns `true` when this error opens a new window (show the single notice), `false`
    /// when it is part of one (show the summary of [`ErrorDigest::count`]).
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

/// Feeds the anime notifications to a sink. Created once per queue; the engine calls the progress method.
pub struct AnimeNotifier {
    sink: Arc<dyn NotificationSink>,
    hide_content: Arc<AtomicBool>,
    last_progress: Option<Instant>,
    errors: ErrorDigest,
}

impl AnimeNotifier {
    pub fn new(sink: Arc<dyn NotificationSink>, hide_content: Arc<AtomicBool>) -> Self {
        Self { sink, hide_content, last_progress: None, errors: ErrorDigest::default() }
    }

    fn hide(&self) -> bool {
        self.hide_content.load(Ordering::Relaxed)
    }

    /// Shows the progress line. Without `state_changed` it is skipped while the last update is under one second old.
    /// Returns `true` when a notice was sent.
    pub fn progress(
        &mut self,
        now: Instant,
        anime_title: &str,
        episode_name: &str,
        percent: i32,
        state_changed: bool,
    ) -> bool {
        let due = self.last_progress.is_none_or(|t| now.saturating_duration_since(t) >= PROGRESS_INTERVAL);
        if !state_changed && !due {
            return false;
        }
        self.last_progress = Some(now);
        let notice = progress_notice(anime_title, episode_name, percent, self.hide());
        self.sink.show(&notice);
        true
    }

    /// The run finished: the done notice replaces any progress line.
    pub fn complete(&mut self) {
        self.last_progress = None;
        self.sink.show(&done_notice());
    }

    /// The queue was cleared: the progress line goes away and nothing is announced.
    pub fn dismiss(&mut self) {
        self.last_progress = None;
        self.sink.withdraw(PROGRESS_ID);
    }

    /// A non-fatal reason to stop, such as "No Wi-Fi connection available", with an optional help link.
    pub fn warning(&mut self, text: &str, help_url: Option<&'static str>) {
        self.sink.show(&warning_notice(text, help_url));
    }

    /// One episode failed. The first in a window is shown alone, later ones in the window are summarised.
    pub fn error(&mut self, now: Instant, anime_title: Option<&str>, episode: Option<&str>, message: &str) {
        if self.errors.record(now) {
            self.sink.show(&error_notice(anime_title, episode, message));
        } else {
            self.sink.show(&error_summary_notice(self.errors.count()));
        }
    }

    /// Maps a queue event to its notification. Events the anime queue does not show are ignored.
    pub fn dispatch(&mut self, now: Instant, event: &DownloadEvent) {
        match event {
            DownloadEvent::Error { manga_title, chapter_name, message, .. } => {
                self.error(now, manga_title.as_deref(), chapter_name.as_deref(), message);
            }
            DownloadEvent::Warning { text, help_url, .. } => self.warning(text, known_help_url(help_url.as_deref())),
            DownloadEvent::Complete => self.complete(),
            DownloadEvent::Dismiss => self.dismiss(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<Notice>>, Mutex<Vec<&'static str>>);

    impl NotificationSink for Recorder {
        fn show(&self, notice: &Notice) {
            self.0.lock().unwrap().push(notice.clone());
        }
        fn withdraw(&self, id: &'static str) {
            self.1.lock().unwrap().push(id);
        }
    }

    fn notifier(hide: bool) -> (AnimeNotifier, Arc<Recorder>) {
        let rec = Arc::new(Recorder::default());
        let sink: Arc<dyn NotificationSink> = rec.clone();
        (AnimeNotifier::new(sink, Arc::new(AtomicBool::new(hide))), rec)
    }

    #[test]
    fn progress_text_is_indeterminate_at_zero() {
        assert_eq!(progress_text(0), "Downloading…");
        assert_eq!(progress_text(42), "Downloading (42%)");
    }

    #[test]
    fn progress_title_strips_the_untruncated_episode_prefix() {
        // E-U01: the prefix is removed with the full title, then the title is cut to 15 and the line to 30.
        let n = progress_notice("A very long anime title here", "A very long anime title here - Episode 3", 10, false);
        assert_eq!(n.title, "A very long an… - Episode 3");
        assert_eq!(n.body.as_deref(), Some("Downloading (10%)"));
        assert!(n.ongoing);
        assert_eq!(n.actions, vec![Action::Stop, Action::ShowAnime]);
    }

    #[test]
    fn hidden_content_puts_the_progress_text_in_the_title() {
        let n = progress_notice("Show", "Episode 1", 0, true);
        assert_eq!(n.title, "Downloading…");
        assert_eq!(n.body, None);
    }

    #[test]
    fn progress_updates_are_rate_limited_to_one_per_second() {
        let (mut n, rec) = notifier(false);
        let t0 = Instant::now();
        assert!(n.progress(t0, "Show", "Episode 1", 1, false));
        assert!(!n.progress(t0 + Duration::from_millis(400), "Show", "Episode 1", 2, false));
        assert!(n.progress(t0 + Duration::from_millis(1100), "Show", "Episode 1", 3, false));
        assert!(n.progress(t0 + Duration::from_millis(1200), "Show", "Episode 1", 4, true), "a state change is never held back");
        assert_eq!(rec.0.lock().unwrap().len(), 3);
    }

    #[test]
    fn the_end_shows_the_done_notice_and_a_dismiss_withdraws() {
        let (mut n, rec) = notifier(false);
        n.complete();
        n.dismiss();
        let shown = rec.0.lock().unwrap();
        assert_eq!(shown[0].id, PROGRESS_ID);
        assert_eq!(shown[0].title, "Downloads finished");
        assert!(shown[0].actions.is_empty());
        assert_eq!(*rec.1.lock().unwrap(), vec![PROGRESS_ID]);
    }

    #[test]
    fn warning_uses_the_error_slot_and_expires_after_thirty_seconds() {
        let w = warning_notice("No Wi-Fi connection available", None);
        assert!(w.actions.is_empty(), "no help link, no action");
        assert_eq!(w.id, ERROR_ID);
        assert_eq!(w.title, "Downloader");
        assert_eq!(w.expire_secs, Some(30));
        assert!(!w.ongoing);
    }

    #[test]
    fn a_single_error_names_the_episode_and_a_second_one_summarises() {
        let (mut n, rec) = notifier(false);
        let t0 = Instant::now();
        n.error(t0, Some("Show"), Some("Episode 2"), "HTTP error 500");
        n.error(t0 + Duration::from_secs(2), Some("Show"), Some("Episode 3"), "timeout");
        let shown = rec.0.lock().unwrap();
        assert_eq!(shown[0].title, "Show: Episode 2");
        assert_eq!(shown[0].body.as_deref(), Some("HTTP error 500"));
        assert_eq!(shown[1].body.as_deref(), Some("2 episode downloads failed"));
        assert!(shown.iter().all(|x| x.id == ERROR_ID));
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
    fn a_queue_size_warning_offers_the_help_link_and_others_do_not() {
        let url = crate::downloader::BULK_WARNING_HELP_URL;
        assert_eq!(warning_notice("Large bulk downloads", Some(url)).actions, vec![Action::OpenHelp(url)]);
        assert_eq!(known_help_url(Some(url)), Some(url));
        assert_eq!(known_help_url(Some("https://example.org/unknown")), None, "only known help links are offered");
        assert_eq!(known_help_url(None), None);
    }

    #[test]
    fn the_anime_actions_use_the_kotlin_labels() {
        assert_eq!(Action::Stop.label(), "Stop");
        assert_eq!(Action::Pause.label(), "Pause");
        assert_eq!(Action::ShowAnime.label(), "Show anime");
        assert_eq!(Action::Resume.label(), "Resume");
        assert_eq!(Action::CancelAll.label(), "Cancel all");
        assert_eq!(Action::OpenHelp("https://x").label(), "Help");
    }

    #[test]
    fn errors_without_a_title_or_message_fall_back_to_the_plain_text() {
        let bare = error_notice(None, None, "");
        assert_eq!(bare.title, "Downloader");
        assert_eq!(bare.body.as_deref(), Some(UNKNOWN_ERROR_TEXT));
        assert!(bare.actions.is_empty());
        assert!(!bare.title.contains("null"));
    }

    #[test]
    fn dispatch_maps_queue_events_and_ignores_the_rest() {
        let (mut n, rec) = notifier(false);
        let t0 = Instant::now();
        n.dispatch(t0, &DownloadEvent::Started);
        n.dispatch(t0, &DownloadEvent::Warning { text: "No network connection available".into(), timeout_secs: Some(30), help_url: None });
        n.dispatch(t0, &DownloadEvent::Paused);
        n.dispatch(t0, &DownloadEvent::Complete);
        n.dispatch(t0, &DownloadEvent::Dismiss);
        let shown = rec.0.lock().unwrap();
        assert_eq!(shown.len(), 2, "the warning and the done notice; a pause shows nothing");
        assert_eq!(shown[0].body.as_deref(), Some("No network connection available"));
        assert_eq!(shown[1].title, "Downloads finished");
        assert_eq!(*rec.1.lock().unwrap(), vec![PROGRESS_ID]);
    }
}
