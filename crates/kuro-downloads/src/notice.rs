//! The notice types the downloader posts, and the [`NotificationSink`] the GUI implements over the platform
//! [`Notifier`]. Which events become which notices is `next::notify`.

use std::sync::Arc;

use kuro_platform::notifier::Notifier;

pub const DOWNLOADER_TITLE: &str = "Downloader";
/// The one notice a finished download posts (user, 2026-10-12: only the end is announced, besides failures).
pub const DONE_TITLE: &str = "Downloads finished";
pub const UNKNOWN_ERROR_TEXT: &str = "Could not download chapter due to unexpected error";

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
    /// The anime progress line's pause button, labelled "Stop" (T-24a-064). It pauses the run like [`Action::Pause`].
    Stop,
    Resume,
    CancelAll,
    ShowManga,
    ShowAnime,
    /// Opens the help link of a warning in the default browser (T-24a-065). Carries the link.
    OpenHelp(&'static str),
}

impl Action {
    /// The button text. Handled inside the app: the GUI maps each action to a desktop action, not a broadcast.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pause => "Pause",
            Self::Stop => "Stop",
            Self::Resume => "Resume",
            Self::CancelAll => "Cancel all",
            Self::ShowManga => "Show manga",
            Self::ShowAnime => "Show anime",
            Self::OpenHelp(_) => "Help",
        }
    }
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

/// A [`NotificationSink`] over the platform [`Notifier`]. The text is already final: the notice builders hide names under `hide_content` ("Downloading (x/y)", empty body), so the notice goes out as built.
/// The platform's blanket replacement (`send` with `sensitive`) is not used: it would overwrite the counts with a
/// generic text (T-23a-043, T-24a-028).
pub struct NotifierSink {
    notifier: Arc<dyn Notifier>,
}

impl NotifierSink {
    pub fn new(notifier: Arc<dyn Notifier>) -> Self {
        Self { notifier }
    }
}

impl NotificationSink for NotifierSink {
    fn show(&self, notice: &Notice) {
        self.notifier.show(notice.id, &notice.title, notice.body.as_deref());
    }

    fn withdraw(&self, id: &'static str) {
        self.notifier.withdraw(id);
    }
}
