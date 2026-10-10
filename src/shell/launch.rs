//! Launch requests through GApplication (T-53a-006, T-53a-007, T-36a-001). The first launch and every later launch
//! deliver their command line through the `command-line` signal, and files from a file manager arrive through
//! `open`. A later launch is forwarded by GApplication to the running instance, which applies the requests to its
//! window. The decisions are pure; `connect` and `apply` are thin.

use std::path::PathBuf;

use gtk::prelude::*;
use gtk::{gio, glib};
use kuro_platform::deep_link::{DeepLink, USAGE_EXIT_CODE};

use super::argv::{self, LaunchRequest, RouteError};
use super::uri::{self, UriRoute};
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
    /// Open the restore page with this backup file checked (EC-09). The file is only read after the user confirms.
    Restore(PathBuf),
    /// A repository or store link: open its settings page and hand it the address.
    Repo(UriRoute),
    /// Raise the window; the request has no screen of its own yet.
    Present,
}

/// The window's target for a request. A request that cannot be routed is an error, and the caller shows its message.
pub fn target_of(request: &LaunchRequest) -> Result<Target, RouteError> {
    Ok(match request {
        LaunchRequest::Open(link) => Target::Link(link.clone()),
        LaunchRequest::Action { name, id } => {
            if argv::needs_id(name) && id.is_none() {
                return Err(RouteError::MissingId(name.clone()));
            }
            Target::Link(argv::link_for_action(name, *id).ok_or_else(|| RouteError::UnknownAction(name.clone()))?)
        }
        LaunchRequest::Search { query, kind } => Target::Search { query: query.clone(), kind: *kind },
        LaunchRequest::Invalid(error) => return Err(error.clone()),
        LaunchRequest::OpenFile(path) => Target::Restore(path.clone()),
        LaunchRequest::OpenUri(link) => match uri::parse_uri(link).map_err(RouteError::Uri)? {
            UriRoute::TrackerAuth { .. } => Target::Present,
            route => Target::Repo(route),
        },
    })
}

/// The settings page that restores a backup file (`settings_restore_backup`).
pub const RESTORE_SCREEN: &str = "settings_restore_backup";

/// The request for one opened file: a `.tachibk` backup by path, a web address as a search, another URI scheme by text.
/// A local file of any other kind is `Unrecognised`, which the window shows (EC-10).
pub fn request_for_open(path: Option<PathBuf>, uri: &str) -> Option<LaunchRequest> {
    match path {
        Some(path) if path.extension().is_some_and(|e| e == "tachibk") => Some(LaunchRequest::OpenFile(path)),
        Some(path) => Some(LaunchRequest::Invalid(RouteError::Unrecognised(path.display().to_string()))),
        None if argv::is_web_address(uri) => Some(LaunchRequest::Search { query: uri.to_string(), kind: Kind::Anime }),
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
        Ok(Target::Link(link)) => crate::open_link::dispatch(app, win, &link.to_arg()),
        Ok(Target::Search { query, kind }) => {
            win.present();
            crate::ui::pop_to_root();
            if argv::is_web_address(&query) {
                // EC-11: the address is resolved first; the page falls back to the search of the kind
                super::deep_link_page::open(kind, &query);
            } else {
                super::deep_link_page::open_search(kind, &query);
            }
            if !crate::data::is_installed() {
                put_query(win, &query);
            }
        }
        Ok(Target::Repo(route)) => {
            win.present();
            if let (Some(screen), Some(link)) = (uri::screen_of(&route), repo_address(&route)) {
                crate::ui::pop_to_root();
                uri::park_repo_url(link);
                crate::ui::open(screen);
            }
        }
        Ok(Target::Restore(path)) => {
            win.present();
            crate::pref_model::backup_pages::set_restore_file(path);
            crate::ui::open(RESTORE_SCREEN);
        }
        Ok(Target::Present) => win.present(),
        Err(error) => {
            tracing::warn!("a launch request was refused: {error}");
            win.present();
            crate::toast::notify(error.to_string());
        }
    }
}

/// The address a repository or store route carries.
fn repo_address(route: &UriRoute) -> Option<String> {
    match route {
        UriRoute::MangaRepos { url } | UriRoute::AnimeStores { url } => Some(url.clone()),
        UriRoute::TrackerAuth { .. } => None,
    }
}

/// The spec id of the global search screen.
pub const GLOBAL_SEARCH_SCREEN: &str = "global_search";

/// Puts `query` in the search box of the global search page that was pushed last (the sample page, when no app data is
/// installed).
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
        assert_eq!(target_of(&request), Ok(Target::Link(DeepLink::Updates)));
    }

    #[test]
    fn an_entry_action_opens_that_entry_and_without_an_id_is_a_route_error() {
        assert_eq!(target_of(&LaunchRequest::Action { name: "show-manga".into(), id: Some(4) }), Ok(Target::Link(DeepLink::Manga(4))));
        // E-18: the window shows the message instead of opening nothing
        assert_eq!(
            target_of(&LaunchRequest::Action { name: "show-manga".into(), id: None }),
            Err(RouteError::MissingId("show-manga".into()))
        );
    }

    #[test]
    fn an_unknown_kind_or_action_is_a_route_error_with_a_message() {
        assert_eq!(target_of(&LaunchRequest::Invalid(RouteError::UnknownKind("comic".into()))), Err(RouteError::UnknownKind("comic".into())));
        assert_eq!(
            target_of(&LaunchRequest::Action { name: "show-nothing".into(), id: None }),
            Err(RouteError::UnknownAction("show-nothing".into()))
        );
    }

    #[test]
    fn open_search_and_file_requests_have_their_targets() {
        assert_eq!(target_of(&LaunchRequest::Open(DeepLink::History)), Ok(Target::Link(DeepLink::History)));
        assert_eq!(
            target_of(&LaunchRequest::Search { query: "frieren".into(), kind: Kind::Anime }),
            Ok(Target::Search { query: "frieren".into(), kind: Kind::Anime })
        );
        assert_eq!(target_of(&LaunchRequest::OpenFile("/tmp/a.tachibk".into())), Ok(Target::Restore("/tmp/a.tachibk".into())));
        assert_eq!(target_of(&LaunchRequest::OpenUri("kurobayomi://anilist-auth#access_token=a".into())), Ok(Target::Present));
    }

    #[test]
    fn repo_links_route_and_bad_links_are_errors_with_a_toast_message() {
        let link = "aniyomi://add-repo?url=https%3A%2F%2Fa.test%2Fi.json";
        assert_eq!(
            target_of(&LaunchRequest::OpenUri(link.into())),
            Ok(Target::Repo(UriRoute::AnimeStores { url: "https://a.test/i.json".into() }))
        );
        let unknown = target_of(&LaunchRequest::OpenUri("kurobayomi://unknown-host".into()));
        assert_eq!(unknown, Err(RouteError::Uri(uri::UriError::UnknownHost("unknown-host".into()))));
        assert!(!unknown.unwrap_err().to_string().is_empty());
        assert_eq!(
            target_of(&LaunchRequest::OpenUri("ftp://x/y".into())),
            Err(RouteError::Uri(uri::UriError::UnsupportedScheme("ftp".into())))
        );
    }

    #[test]
    fn opened_files_become_requests_by_kind() {
        assert_eq!(request_for_open(Some("/tmp/b.tachibk".into()), "file:///tmp/b.tachibk"), Some(LaunchRequest::OpenFile("/tmp/b.tachibk".into())));
        // EC-10: a file that is not a backup is shown as unrecognised, not dropped
        assert_eq!(
            request_for_open(Some("/tmp/b.tachibk.part".into()), "file:///tmp/b.tachibk.part"),
            Some(LaunchRequest::Invalid(RouteError::Unrecognised("/tmp/b.tachibk.part".into())))
        );
        assert_eq!(
            request_for_open(None, "https://example.test/anime/1"),
            Some(LaunchRequest::Search { query: "https://example.test/anime/1".into(), kind: Kind::Anime })
        );
        assert_eq!(request_for_open(None, "aniyomi://add-repo?url=x"), Some(LaunchRequest::OpenUri("aniyomi://add-repo?url=x".into())));
        assert_eq!(request_for_open(None, "plain"), None);
    }
}
