//! The desktop notices of an update pass (T-26a-031, T-26a-034, T-26a-035): the large-queue warning with its help link,
//! the per-entry "new chapters / episodes" notice with its buttons, and the quiet "N update(s) failed" notice.
//!
//! The runner's host lives on the runtime's workers and GTK objects stay on the main thread, so a notice is a plain
//! [`Post`] sent over a channel to [`attach`], which turns it into a `gio::Notification`. The pure mapping from a pass
//! result to a [`Post`] is tested below; the GTK half is thin.
//!
//! Decisions:
//! - A notification has no timeout of its own, so a [`Post`] with one is withdrawn by a timer on the main context.
//! - The tap of an entry notice opens the entry's page (the `open-manga` / `open-anime` application actions); the
//!   reader or player at the newest new item is not wired because the shell has no route that takes an item url.
//! - The "Download" button queues the named items with autostart on through the downloads backend
//!   (`library::actions::download_notified`); "Mark as read" also removes the downloads of the changed items when
//!   "remove after marked as read" is on. Both run off the GTK thread and withdraw the notice when they end.
//! - The failure notice taps through to the error log (`update_errors.txt`) in the default `text/plain` application. It
//!   has no body and no tap while no log was written, so it never promises a tap that does nothing.

use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use kuro_domain::kind::EntryKind;
use kuro_downloads::download_prefs::DownloadPrefs;

use crate::library_update::notice::{self, EntryAction, ERROR_CHANNEL, WARNING_CHANNEL};
use crate::library_update::pass::EntryUpdate;
use crate::library_update::text;
use crate::updates::download_host;

/// Application action that opens the url it is given.
const OPEN_URL_ACTION: &str = "library-update-open-url";
/// Application action that marks the items of one update notice read; takes `(kind, entry id, urls, notice id)`.
const MARK_READ_ACTION: &str = "library-update-mark-read";
/// Application action that queues the items of one update notice for download; takes the same tuple.
const DOWNLOAD_ACTION: &str = "library-update-download";
/// Application action that opens the update error log in the default text application; takes the file path.
const OPEN_LOG_ACTION: &str = "library-update-open-log";

/// What a notification button or tap does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Entry { kind: EntryKind, id: i64 },
    Url(String),
    MarkRead { kind: EntryKind, entry_id: i64, urls: Vec<String>, notice_id: String },
    Download { kind: EntryKind, entry_id: i64, urls: Vec<String>, notice_id: String },
    /// The error log file.
    OpenLog(String),
}

fn kind_name(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Manga => "manga",
        EntryKind::Anime => "anime",
    }
}

fn kind_of(name: &str) -> Option<EntryKind> {
    match name {
        "manga" => Some(EntryKind::Manga),
        "anime" => Some(EntryKind::Anime),
        _ => None,
    }
}

impl Target {
    /// The detailed application action name and its parameter.
    fn action(&self) -> (String, glib::Variant) {
        match self {
            Self::Entry { kind, id } => (format!("app.open-{}", kind_name(*kind)), id.to_variant()),
            Self::Url(url) => (format!("app.{OPEN_URL_ACTION}"), url.to_variant()),
            Self::MarkRead { kind, entry_id, urls, notice_id } => (
                format!("app.{MARK_READ_ACTION}"),
                (kind_name(*kind).to_owned(), *entry_id, urls.clone(), notice_id.clone()).to_variant(),
            ),
            Self::Download { kind, entry_id, urls, notice_id } => (
                format!("app.{DOWNLOAD_ACTION}"),
                (kind_name(*kind).to_owned(), *entry_id, urls.clone(), notice_id.clone()).to_variant(),
            ),
            Self::OpenLog(path) => (format!("app.{OPEN_LOG_ACTION}"), path.to_variant()),
        }
    }
}

/// One notification to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Post {
    pub id: String,
    pub title: String,
    pub body: Option<String>,
    pub low_priority: bool,
    /// Withdrawn after this long.
    pub timeout: Option<Duration>,
    pub tap: Option<Target>,
    pub buttons: Vec<(String, Target)>,
}

fn notice_id(channel: &str, id: u32) -> String {
    format!("library-update-{channel}-{id}")
}

/// The per-entry notice of an update, or `None` when the content is hidden or nothing is new (T-26a-034).
pub fn entry_post(kind: EntryKind, update: &EntryUpdate, hide_content: bool) -> Option<Post> {
    let entry = notice::entry_notice(kind, update, hide_content)?;
    let id = notice_id(kind_name(kind), entry.id);
    let buttons = entry
        .actions
        .iter()
        .filter_map(|action| {
            let target = match action {
                EntryAction::MarkRead => Target::MarkRead {
                    kind,
                    entry_id: update.entry_id,
                    urls: update.items.iter().map(|i| i.url.clone()).collect(),
                    notice_id: id.clone(),
                },
                EntryAction::ViewItems => Target::Entry { kind, id: update.entry_id },
                EntryAction::Download => Target::Download {
                    kind,
                    entry_id: update.entry_id,
                    urls: update.items.iter().map(|i| i.url.clone()).collect(),
                    notice_id: id.clone(),
                },
            };
            Some((action.label(kind).to_owned(), target))
        })
        .collect();
    Some(Post {
        id,
        title: entry.title,
        body: Some(entry.text),
        low_priority: false,
        timeout: None,
        tap: Some(Target::Entry { kind, id: update.entry_id }),
        buttons,
    })
}

/// The "N update(s) failed" notice: low urgency, shown only when something failed (T-26a-035). With a `log` file its
/// tap opens the file and its body says so (T-26a-096); without one it has neither.
pub fn error_post(kind: EntryKind, failed: usize, log: Option<&Path>) -> Option<Post> {
    let error = notice::error_notice(failed, log.is_some())?;
    Some(Post {
        id: notice_id(ERROR_CHANNEL, notice::pass_id(ERROR_CHANNEL, kind)),
        title: error.title,
        body: error.opens_log.then(|| error.text.to_owned()),
        low_priority: true,
        timeout: None,
        tap: log.map(|path| Target::OpenLog(path.to_string_lossy().into_owned())),
        buttons: Vec::new(),
    })
}

/// The large-queue warning: stays 30 s and its tap opens the FAQ page. Hidden content does not suppress it (T-26a-031).
pub fn warning_post(kind: EntryKind) -> Post {
    Post {
        id: notice_id(WARNING_CHANNEL, notice::pass_id(WARNING_CHANNEL, kind)),
        title: text::QUEUE_WARNING_TITLE.to_owned(),
        body: Some(text::QUEUE_WARNING_TEXT.to_owned()),
        low_priority: false,
        timeout: Some(Duration::from_millis(text::QUEUE_WARNING_TIMEOUT_MS)),
        tap: Some(Target::Url(text::QUEUE_WARNING_URL.to_owned())),
        buttons: Vec::new(),
    }
}

static SENDER: OnceLock<async_channel::Sender<Post>> = OnceLock::new();

/// Sends `post` to the main thread. `false` when [`attach`] has not run, so the caller can fall back to a toast.
pub fn post(post: Post) -> bool {
    SENDER.get().is_some_and(|tx| tx.try_send(post).is_ok())
}

fn show(app: &gio::Application, post: &Post) {
    let notification = gio::Notification::new(&post.title);
    notification.set_body(post.body.as_deref());
    if post.low_priority {
        notification.set_priority(gio::NotificationPriority::Low);
    }
    if let Some(tap) = &post.tap {
        let (action, target) = tap.action();
        notification.set_default_action_and_target_value(&action, Some(&target));
    }
    for (label, target) in &post.buttons {
        let (action, value) = target.action();
        notification.add_button_with_target_value(label, &action, Some(&value));
    }
    app.send_notification(Some(&post.id), &notification);
    if let Some(timeout) = post.timeout {
        let (app, id) = (app.clone(), post.id.clone());
        glib::timeout_add_local_once(timeout, move || app.withdraw_notification(&id));
    }
}

fn open_url(url: &str) {
    gtk::UriLauncher::new(url).launch(None::<&gtk::Window>, None::<&gio::Cancellable>, |result| {
        if let Err(error) = result {
            tracing::warn!(%error, "the notification link could not be opened");
        }
    });
}

/// The `(kind, entry id, urls, notice id)` of a notice button.
fn notice_params(params: &glib::Variant) -> Option<(EntryKind, i64, Vec<String>, String)> {
    let (kind, entry_id, urls, notice_id) = params.get::<(String, i64, Vec<String>, String)>()?;
    Some((kind_of(&kind)?, entry_id, urls, notice_id))
}

fn mark_read(app: &gio::Application, params: &glib::Variant) {
    let Some((kind, entry_id, urls, notice_id)) = notice_params(params) else { return };
    let data = crate::data::app();
    let db = data.db(kind);
    let remove = DownloadPrefs::new(data.base.store().clone()).remove_after_marked_as_read().get();
    let backend = download_host::current();
    let app = app.clone();
    glib::spawn_future_local(async move {
        let done = download_host::blocking(move || {
            crate::library::actions::mark_notified_read(&db, &*backend, kind, entry_id, &urls, remove)
        })
        .await;
        match done {
            Some(Ok(_)) => app.withdraw_notification(&notice_id),
            Some(Err(error)) => tracing::error!(%error, entry_id, "marking the notified items read failed"),
            None => tracing::error!(entry_id, "marking the notified items read did not finish"),
        }
    });
}

fn download(app: &gio::Application, params: &glib::Variant) {
    let Some((kind, entry_id, urls, notice_id)) = notice_params(params) else { return };
    let data = crate::data::app();
    let db = data.db(kind);
    let external = kind == EntryKind::Anime && DownloadPrefs::new(data.base.store().clone()).use_external_downloader().get();
    let backend = download_host::current();
    let app = app.clone();
    glib::spawn_future_local(async move {
        let queued = download_host::blocking(move || {
            crate::library::actions::download_notified(&db, &*backend, kind, entry_id, &urls, external)
        })
        .await;
        if queued.is_some() {
            app.withdraw_notification(&notice_id);
        }
    });
}

/// Opens the error log with the default `text/plain` application.
fn open_log(path: &str) {
    let Some(viewer) = gio::AppInfo::default_for_type("text/plain", false) else {
        tracing::warn!("no default text application is set, so the update error log was not opened");
        return;
    };
    if let Err(error) = viewer.launch(&[gio::File::for_path(path)], None::<&gio::AppLaunchContext>) {
        tracing::warn!(%error, "the update error log could not be opened");
    }
}

/// Registers the notification actions and starts showing posted notices. Call once from the GTK main thread, with the
/// application running. Without an application (a test run) nothing is attached and [`post`] returns `false`.
pub fn attach() {
    let Some(app) = gio::Application::default() else { return };
    let (tx, rx) = async_channel::unbounded();
    if SENDER.set(tx).is_err() {
        return;
    }
    let open = gio::SimpleAction::new(OPEN_URL_ACTION, Some(glib::VariantTy::STRING));
    open.connect_activate(|_, param| {
        if let Some(url) = param.and_then(|p| p.get::<String>()) {
            open_url(&url);
        }
    });
    app.add_action(&open);
    let mark = gio::SimpleAction::new(
        MARK_READ_ACTION,
        Some(glib::VariantTy::new("(sxass)").expect("a valid variant type")),
    );
    let mark_app = app.clone();
    mark.connect_activate(move |_, param| {
        if let Some(param) = param {
            mark_read(&mark_app, param);
        }
    });
    app.add_action(&mark);
    let download_action = gio::SimpleAction::new(
        DOWNLOAD_ACTION,
        Some(glib::VariantTy::new("(sxass)").expect("a valid variant type")),
    );
    let download_app = app.clone();
    download_action.connect_activate(move |_, param| {
        if let Some(param) = param {
            download(&download_app, param);
        }
    });
    app.add_action(&download_action);
    let log_action = gio::SimpleAction::new(OPEN_LOG_ACTION, Some(glib::VariantTy::STRING));
    log_action.connect_activate(|_, param| {
        if let Some(path) = param.and_then(|p| p.get::<String>()) {
            open_log(&path);
        }
    });
    app.add_action(&log_action);
    glib::spawn_future_local(async move {
        while let Ok(post) = rx.recv().await {
            show(&app, &post);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_update::source::SourceItem;

    fn update(items: &[(f64, i64)]) -> EntryUpdate {
        EntryUpdate {
            entry_id: 12,
            title: "Berserk".into(),
            items: items
                .iter()
                .map(|(number, order)| SourceItem {
                    url: format!("/c{number}"),
                    number: *number,
                    source_order: *order,
                    ..SourceItem::default()
                })
                .collect(),
        }
    }

    #[test]
    fn the_warning_lasts_30_seconds_and_its_tap_opens_the_faq() {
        // T-26a-031
        let post = warning_post(EntryKind::Manga);
        assert_eq!(post.timeout, Some(Duration::from_secs(30)));
        assert_eq!(post.tap, Some(Target::Url(text::QUEUE_WARNING_URL.to_owned())));
        assert_eq!(post.body.as_deref(), Some(text::QUEUE_WARNING_TEXT));
        assert_ne!(post.id, warning_post(EntryKind::Anime).id);
    }

    #[test]
    fn the_entry_notice_offers_mark_read_and_view_and_taps_into_the_entry() {
        // T-26a-034
        let post = entry_post(EntryKind::Manga, &update(&[(1.0, 1), (2.0, 0)]), false).expect("shown");
        let labels: Vec<&str> = post.buttons.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["Mark as read", "View chapters", "Download"]);
        assert_eq!(post.tap, Some(Target::Entry { kind: EntryKind::Manga, id: 12 }));
        let Target::MarkRead { urls, notice_id, .. } = &post.buttons[0].1 else { panic!("mark read first") };
        assert_eq!(urls, &["/c1".to_owned(), "/c2".to_owned()]);
        assert_eq!(notice_id, &post.id);
        let Target::Download { urls, .. } = &post.buttons[2].1 else { panic!("download last") };
        assert_eq!(urls, &["/c1".to_owned(), "/c2".to_owned()], "the button names the new items");
        let anime = entry_post(EntryKind::Anime, &update(&[(1.0, 0)]), false).expect("shown");
        assert_eq!(anime.buttons[0].0, "Mark as watched");
        assert_eq!(anime.buttons[1].0, "View episodes");
    }

    #[test]
    fn a_long_list_of_new_items_drops_the_download_button() {
        // T-26a-094 limits: 15 chapters, 10 episodes
        let many: Vec<(f64, i64)> = (0..16).map(|n| (f64::from(n), i64::from(n))).collect();
        let post = entry_post(EntryKind::Manga, &update(&many), false).expect("shown");
        assert_eq!(post.buttons.len(), 2, "16 chapters: no Download");
        let post = entry_post(EntryKind::Manga, &update(&many[..15]), false).expect("shown");
        assert_eq!(post.buttons.len(), 3, "15 chapters: Download");
        let post = entry_post(EntryKind::Anime, &update(&many[..11]), false).expect("shown");
        assert_eq!(post.buttons.len(), 2, "11 episodes: no Download");
    }

    #[test]
    fn hidden_content_suppresses_the_entry_notice_but_not_the_warning() {
        assert!(entry_post(EntryKind::Manga, &update(&[(1.0, 0)]), true).is_none());
        assert!(entry_post(EntryKind::Manga, &update(&[]), false).is_none());
        assert_eq!(warning_post(EntryKind::Manga).title, text::QUEUE_WARNING_TITLE);
    }

    #[test]
    fn the_failure_notice_is_low_and_only_for_failures() {
        // T-26a-035
        assert!(error_post(EntryKind::Manga, 0, None).is_none());
        let post = error_post(EntryKind::Manga, 3, None).expect("shown");
        assert_eq!(post.title, "3 update(s) failed");
        assert!(post.low_priority);
        assert_eq!(post.body, None, "no log file, so no tap line");
        assert_eq!(post.tap, None);
    }

    #[test]
    fn the_failure_notice_opens_the_error_log_when_one_was_written() {
        // T-26a-096
        let log = Path::new("/cache/update_errors.txt");
        let post = error_post(EntryKind::Anime, 2, Some(log)).expect("shown");
        assert_eq!(post.body.as_deref(), Some("Tap to see details"));
        assert_eq!(post.tap, Some(Target::OpenLog("/cache/update_errors.txt".to_owned())));
        let (action, value) = post.tap.expect("tap").action();
        assert_eq!(action, "app.library-update-open-log");
        assert_eq!(value.get::<String>().as_deref(), Some("/cache/update_errors.txt"));
    }

    #[test]
    fn a_target_names_its_application_action() {
        let (action, value) = Target::Entry { kind: EntryKind::Anime, id: 5 }.action();
        assert_eq!(action, "app.open-anime");
        assert_eq!(value.get::<i64>(), Some(5));
        let (action, value) = Target::Url("https://x".into()).action();
        assert_eq!(action, "app.library-update-open-url");
        assert_eq!(value.get::<String>().as_deref(), Some("https://x"));
        let download = Target::Download { kind: EntryKind::Manga, entry_id: 2, urls: vec!["/a".into()], notice_id: "n".into() };
        assert_eq!(download.action().0, "app.library-update-download");
        let mark = Target::MarkRead { kind: EntryKind::Manga, entry_id: 2, urls: vec!["/a".into()], notice_id: "n".into() };
        let (action, value) = mark.action();
        assert_eq!(action, "app.library-update-mark-read");
        assert_eq!(value.get::<(String, i64, Vec<String>, String)>(), Some(("manga".into(), 2, vec!["/a".into()], "n".into())));
    }

    #[test]
    fn nothing_is_sent_before_the_notifier_is_attached() {
        assert!(!post(warning_post(EntryKind::Manga)));
    }
}
