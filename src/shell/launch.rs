//! Launch requests through GApplication (T-53a-006, T-53a-007, T-36a-001). The first launch and every later launch
//! deliver their command line through the `command-line` signal, and files from a file manager arrive through
//! `open`. A later launch is forwarded by GApplication to the running instance, which applies the requests to its
//! window. The decisions are pure; `connect` and `apply` are thin.

use std::path::PathBuf;

use gtk::prelude::*;
use gtk::{gio, glib};
use kuro_platform::deep_link::{DeepLink, USAGE_EXIT_CODE};

use super::argv::{self, LaunchRequest};
use super::host::Shell;
use super::nav::Kind;

/// Application flags: the command line and file opens are handled here, and a dev run (`unique` false) does not
/// hand its command to a running window.
pub fn app_flags(unique: bool) -> gio::ApplicationFlags {
    let mut flags = gio::ApplicationFlags::HANDLES_COMMAND_LINE | gio::ApplicationFlags::HANDLES_OPEN;
    if !unique {
        flags |= gio::ApplicationFlags::NON_UNIQUE;
    }
    flags
}

/// What the window does for a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// Pop to the root and open a place (the `app.open-*` actions).
    Link(DeepLink),
    /// Open global search with the text in its entry.
    Search { query: String, kind: Kind },
    /// Raise the window; the request has no screen of its own yet.
    Present,
}

pub fn target_of(request: &LaunchRequest) -> Target {
    match request {
        LaunchRequest::Open(link) => Target::Link(link.clone()),
        LaunchRequest::Action { name, id } => argv::link_for_action(name, *id).map_or(Target::Present, Target::Link),
        LaunchRequest::Search { query, kind } => Target::Search { query: query.clone(), kind: *kind },
        LaunchRequest::OpenFile(_) | LaunchRequest::OpenUri(_) => Target::Present,
    }
}

/// The request for one opened file: a `.tachibk` backup by path, another URI scheme by text. Local files of any
/// other kind give `None`.
pub fn request_for_open(path: Option<PathBuf>, uri: &str) -> Option<LaunchRequest> {
    match path {
        Some(path) if path.extension().is_some_and(|e| e == "tachibk") => Some(LaunchRequest::OpenFile(path)),
        Some(_) => None,
        None if uri.contains("://") => Some(LaunchRequest::OpenUri(uri.to_string())),
        None => None,
    }
}

/// Connects the `command-line` and `open` signals and makes the first window appear for each. `activate` shows or
/// builds the window.
pub fn connect(app: &gtk::Application) {
    app.connect_command_line(|app, cmd| {
        let args: Vec<String> = cmd.arguments().iter().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
        // `main` already refused a bad command line before this process forwarded it, so an error here is not expected
        match argv::parse_launch(&args) {
            Ok(requests) => {
                submit_all(app, requests);
                glib::ExitCode::SUCCESS
            }
            Err(e) => {
                tracing::warn!("a forwarded command line was refused: {e}");
                glib::ExitCode::from(USAGE_EXIT_CODE as u8)
            }
        }
    });
    app.connect_open(|app, files, _hint| {
        let requests = files.iter().filter_map(|f| request_for_open(f.path(), f.uri().as_str())).collect();
        submit_all(app, requests);
    });
}

fn submit_all(app: &gtk::Application, requests: Vec<LaunchRequest>) {
    let Some(shell) = Shell::get() else { return };
    for request in requests {
        if let Some(now) = shell.submit(request) {
            if let Some(win) = window(app) {
                apply(app, &win, &now);
            }
        }
    }
    // builds the window the first time, raises it afterwards
    app.activate();
}

fn window(app: &gtk::Application) -> Option<gtk::ApplicationWindow> {
    app.windows().into_iter().find_map(|w| w.downcast::<gtk::ApplicationWindow>().ok())
}

/// The window exists: applies the requests that arrived before it, in arrival order.
pub fn window_built(app: &gtk::Application, win: &gtk::ApplicationWindow) {
    let Some(shell) = Shell::get() else { return };
    for request in shell.window_built() {
        apply(app, win, &request);
    }
}

/// Applies one request to the window.
pub fn apply(app: &gtk::Application, win: &gtk::ApplicationWindow, request: &LaunchRequest) {
    // the onboarding wizard is not left by a launch request: the window is only raised
    let (names, shown) = crate::ui::page_stack();
    if names.get(shown).is_some_and(|n| n == super::onboarding::PAGE_NAME) {
        win.present();
        return;
    }
    match target_of(request) {
        Target::Link(link) => crate::open_link::dispatch(app, win, &link.to_arg()),
        Target::Search { query, kind: _ } => {
            win.present();
            crate::ui::pop_to_root();
            crate::ui::open(GLOBAL_SEARCH_SCREEN);
            put_query(win, &query);
        }
        Target::Present => {
            if let LaunchRequest::OpenFile(path) = request {
                tracing::warn!(file = %path.display(), "a backup file was opened, restore it from Data and storage");
            }
            win.present();
        }
    }
}

/// The spec id of the global search screen.
pub const GLOBAL_SEARCH_SCREEN: &str = "global_search";

/// Puts `query` in the search box of the global search page that was pushed last.
fn put_query(win: &gtk::ApplicationWindow, query: &str) {
    let name = format!("screen:{GLOBAL_SEARCH_SCREEN}");
    if let Some(entry) = last_named(win.upcast_ref(), &name).and_then(|page| first_entry(&page)) {
        entry.set_text(query);
    }
}

fn last_named(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    let mut found = (root.widget_name() == name).then(|| root.clone());
    let mut child = root.first_child();
    while let Some(c) = child {
        if let Some(inner) = last_named(&c, name) {
            found = Some(inner);
        }
        child = c.next_sibling();
    }
    found
}

fn first_entry(root: &gtk::Widget) -> Option<gtk::SearchEntry> {
    if let Some(entry) = root.downcast_ref::<gtk::SearchEntry>() {
        return Some(entry.clone());
    }
    let mut child = root.first_child();
    while let Some(c) = child {
        if let Some(entry) = first_entry(&c) {
            return Some(entry);
        }
        child = c.next_sibling();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_handle_the_command_line_and_opens_and_a_dev_run_is_not_unique() {
        let unique = app_flags(true);
        assert!(unique.contains(gio::ApplicationFlags::HANDLES_COMMAND_LINE));
        assert!(unique.contains(gio::ApplicationFlags::HANDLES_OPEN));
        assert!(!unique.contains(gio::ApplicationFlags::NON_UNIQUE));
        assert!(app_flags(false).contains(gio::ApplicationFlags::NON_UNIQUE));
    }

    #[test]
    fn show_updates_opens_the_updates_place() {
        let request = LaunchRequest::Action { name: "show-updates".into(), id: None };
        assert_eq!(target_of(&request), Target::Link(DeepLink::Updates));
    }

    #[test]
    fn an_entry_action_opens_that_entry_and_without_an_id_only_raises_the_window() {
        assert_eq!(target_of(&LaunchRequest::Action { name: "show-manga".into(), id: Some(4) }), Target::Link(DeepLink::Manga(4)));
        assert_eq!(target_of(&LaunchRequest::Action { name: "show-manga".into(), id: None }), Target::Present);
    }

    #[test]
    fn open_search_and_file_requests_have_their_targets() {
        assert_eq!(target_of(&LaunchRequest::Open(DeepLink::History)), Target::Link(DeepLink::History));
        assert_eq!(
            target_of(&LaunchRequest::Search { query: "frieren".into(), kind: Kind::Anime }),
            Target::Search { query: "frieren".into(), kind: Kind::Anime }
        );
        assert_eq!(target_of(&LaunchRequest::OpenFile("/tmp/a.tachibk".into())), Target::Present);
        assert_eq!(target_of(&LaunchRequest::OpenUri("aniyomi://x".into())), Target::Present);
    }

    #[test]
    fn opened_files_become_requests_by_kind() {
        assert_eq!(request_for_open(Some("/tmp/b.tachibk".into()), "file:///tmp/b.tachibk"), Some(LaunchRequest::OpenFile("/tmp/b.tachibk".into())));
        assert_eq!(request_for_open(Some("/tmp/notes.txt".into()), "file:///tmp/notes.txt"), None);
        assert_eq!(request_for_open(None, "aniyomi://add-repo?url=x"), Some(LaunchRequest::OpenUri("aniyomi://add-repo?url=x".into())));
        assert_eq!(request_for_open(None, "plain"), None);
    }
}
