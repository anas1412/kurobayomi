//! The route table of the launch intents (area 36, behaviour 4.1 to 4.4). [`route_request`] is pure: it returns the
//! navigation steps in order, the notification to withdraw and whether the window may be shown. A request whose `ready`
//! is false is not for the window (Kotlin `return false`).

use std::path::{Path, PathBuf};

use kuro_platform::deep_link::DeepLink;

use crate::shell::argv::{self, LaunchRequest, RouteError};
use kuro_platform::kind::Kind;
use crate::shell::uri::{self, UriError, UriRoute};

/// The extras of a notification dismiss (`notificationId` and `groupId`, behaviour 4.1 step 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dismissal {
    pub id: i32,
    /// The group summary's id, 0 when absent. Notices posted on Linux have no group summary, so it is kept and not acted on.
    pub group_id: i32,
}

/// The dismiss extras as an intent carries them. A missing id or a negative one (Kotlin `> -1`) dismisses nothing.
pub fn dismissal(notification_id: Option<i32>, group_id: Option<i32>) -> Option<Dismissal> {
    let id = notification_id.filter(|id| *id >= 0)?;
    Some(Dismissal { id, group_id: group_id.unwrap_or(0) })
}

/// One inbound request. `Launch` wraps the requests the argv, URI and GApplication code already parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    Launch(LaunchRequest),
    Dismiss(Dismissal),
    /// An extension-driven search (Kotlin `INTENT_SEARCH` and `INTENT_ANIMESEARCH`).
    ExtSearch { kind: Kind, query: String, filter: Option<String> },
}

/// A page a navigation step pushes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    GlobalSearch { kind: Kind, query: String, filter: Option<String> },
    DeepLink { kind: Kind, query: String },
    RestoreBackup(PathBuf),
    ExtensionStores { url: String },
}

/// One navigation step, applied in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavAction {
    PopToRoot,
    /// Opens a place (the `app.open-*` actions, `open_link::dispatch`).
    Open(DeepLink),
    Push(Screen),
}

/// What routing decided for one request.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Routed {
    pub actions: Vec<NavAction>,
    /// The notification to withdraw.
    pub withdraw: Option<i32>,
    /// A request the route table refuses; the window shows its message.
    pub error: Option<RouteError>,
    /// The window may be shown.
    pub ready: bool,
}

impl Routed {
    fn to(actions: Vec<NavAction>) -> Self {
        Self { actions, ready: true, ..Self::default() }
    }

    fn failed(error: RouteError, ready: bool) -> Self {
        Self { error: Some(error), ready, ..Self::default() }
    }
}

/// The shortcut actions of the `.desktop` file (the 11 Kotlin `Constants` shortcuts, behaviour 4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutAction {
    AnimeLib,
    Library,
    Anime,
    Manga,
    RecentlyUpdated,
    RecentlyRead,
    Catalogues,
    AnimeExtensions,
    Extensions,
    Downloads,
    AnimeDownloads,
}

impl ShortcutAction {
    pub fn from_desktop(name: &str) -> Option<Self> {
        Some(match name {
            "show-library" => Self::Library,
            "show-animelib" => Self::AnimeLib,
            "show-updates" => Self::RecentlyUpdated,
            "show-history" => Self::RecentlyRead,
            "show-browse" => Self::Catalogues,
            "show-extensions" => Self::Extensions,
            "show-anime-extensions" => Self::AnimeExtensions,
            "show-downloads" => Self::Downloads,
            "show-anime-downloads" => Self::AnimeDownloads,
            "show-manga" => Self::Manga,
            "show-anime" => Self::Anime,
            _ => return None,
        })
    }

    fn desktop_name(self) -> &'static str {
        match self {
            Self::Library => "show-library",
            Self::AnimeLib => "show-animelib",
            Self::RecentlyUpdated => "show-updates",
            Self::RecentlyRead => "show-history",
            Self::Catalogues => "show-browse",
            Self::Extensions => "show-extensions",
            Self::AnimeExtensions => "show-anime-extensions",
            Self::Downloads => "show-downloads",
            Self::AnimeDownloads => "show-anime-downloads",
            Self::Manga => "show-manga",
            Self::Anime => "show-anime",
        }
    }

    /// The place this shortcut opens. An entry shortcut without an id is `MissingId`.
    pub fn link(self, id: Option<i64>) -> Result<DeepLink, RouteError> {
        argv::link_for_action(self.desktop_name(), id).ok_or_else(|| RouteError::MissingId(self.desktop_name().into()))
    }

    /// Entry and download shortcuts pop the stack to the root first (behaviour 4.1 table).
    fn pops_to_root(self) -> bool {
        matches!(self, Self::Manga | Self::Anime | Self::Downloads | Self::AnimeDownloads)
    }
}

/// Routes one request. Pure: the window applies the result.
pub fn route_request(intent: &Intent) -> Routed {
    match intent {
        Intent::Dismiss(dismissal) => Routed { withdraw: Some(dismissal.id), ..Routed::to(vec![]) },
        Intent::ExtSearch { kind, query, filter } => Routed::to(global_search(*kind, query, filter.clone(), false)),
        Intent::Launch(request) => launch(request),
    }
}

fn launch(request: &LaunchRequest) -> Routed {
    match request {
        LaunchRequest::Action { name, id } => shortcut(name, *id),
        LaunchRequest::Open(link) => Routed::to(vec![NavAction::Open(link.clone())]),
        LaunchRequest::Search { query, kind } => Routed::to(global_search(*kind, query, None, true)),
        LaunchRequest::OpenFile(path) => Routed::to(restore(path)),
        LaunchRequest::OpenUri(link) => open_uri(link),
        LaunchRequest::Invalid(error) => Routed::failed(error.clone(), true),
    }
}

fn shortcut(name: &str, id: Option<i64>) -> Routed {
    let Some(action) = ShortcutAction::from_desktop(name) else {
        return Routed::failed(RouteError::UnknownAction(name.into()), false);
    };
    match action.link(id) {
        // a missing id still shows the window, so the message is visible (E-18)
        Err(error) => Routed::failed(error, true),
        Ok(link) if action.pops_to_root() => Routed::to(vec![NavAction::PopToRoot, NavAction::Open(link)]),
        Ok(link) => Routed::to(vec![NavAction::Open(link)]),
    }
}

/// The global search of `kind`, then (for a search request) the deep-link page of the same query on top. An empty query
/// does nothing.
fn global_search(kind: Kind, query: &str, filter: Option<String>, deep_link: bool) -> Vec<NavAction> {
    if query.is_empty() {
        return vec![];
    }
    let mut out = vec![NavAction::PopToRoot, NavAction::Push(Screen::GlobalSearch { kind, query: query.into(), filter })];
    if deep_link {
        out.push(NavAction::Push(Screen::DeepLink { kind, query: query.into() }));
    }
    out
}

/// A backup file opens the restore page. Any other file is ignored.
fn restore(path: &Path) -> Vec<NavAction> {
    if path.extension().is_some_and(|e| e == "tachibk") {
        vec![NavAction::PopToRoot, NavAction::Push(Screen::RestoreBackup(path.to_path_buf()))]
    } else {
        vec![]
    }
}

fn open_uri(link: &str) -> Routed {
    match uri::parse_uri(link) {
        Ok(UriRoute::MangaRepos { url } | UriRoute::AnimeStores { url }) => Routed::to(store(url)),
        // the tracker lane takes its own sign-in callbacks
        Ok(UriRoute::TrackerAuth { .. }) => Routed::to(vec![]),
        // a repository link without an address does nothing (behaviour 4.4)
        Err(UriError::MissingRepositoryAddress) => Routed::to(vec![]),
        Err(error) => Routed::failed(RouteError::Uri(error), true),
    }
}

fn store(url: String) -> Vec<NavAction> {
    vec![NavAction::PopToRoot, NavAction::Push(Screen::ExtensionStores { url })]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch_of(request: LaunchRequest) -> Routed {
        route_request(&Intent::Launch(request))
    }

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_notification_dismiss_withdraws_its_id_with_no_navigation_and_shows_the_window() {
        // T-36a-010: the group id defaults to 0 when absent, and the request has no NavAction
        let d = dismissal(Some(7), None).unwrap();
        assert_eq!(d, Dismissal { id: 7, group_id: 0 });
        let routed = route_request(&Intent::Dismiss(d));
        assert_eq!(routed, Routed { withdraw: Some(7), ..Routed::to(vec![]) });
        assert!(routed.actions.is_empty() && routed.ready);
    }

    #[test]
    fn a_dismiss_without_an_id_or_with_a_negative_one_is_no_dismiss() {
        assert_eq!(dismissal(None, Some(3)), None);
        assert_eq!(dismissal(Some(-1), None), None, "the Kotlin check is greater than -1");
        assert_eq!(dismissal(Some(0), Some(9)), Some(Dismissal { id: 0, group_id: 9 }));
    }

    #[test]
    fn every_desktop_shortcut_maps_and_there_are_eleven_of_them() {
        let mapped: Vec<ShortcutAction> = argv::ACTIONS.iter().filter_map(|name| ShortcutAction::from_desktop(name)).collect();
        assert_eq!(mapped.len(), 11);
        assert_eq!(ShortcutAction::from_desktop("show-nothing"), None);
    }

    #[test]
    fn a_plain_shortcut_opens_its_place_without_popping_and_an_entry_shortcut_pops_first() {
        assert_eq!(launch_of(LaunchRequest::Action { name: "show-library".into(), id: None }).actions, vec![NavAction::Open(DeepLink::Library)]);
        assert_eq!(
            launch_of(LaunchRequest::Action { name: "show-manga".into(), id: Some(3) }).actions,
            vec![NavAction::PopToRoot, NavAction::Open(DeepLink::Manga(3))]
        );
        assert_eq!(
            launch_of(LaunchRequest::Action { name: "show-downloads".into(), id: None }).actions,
            vec![NavAction::PopToRoot, NavAction::Open(DeepLink::Downloads)]
        );
    }

    #[test]
    fn an_entry_shortcut_without_an_id_is_missing_id_and_still_shows_the_window() {
        // E3 of the edge-case list and T-36a-011
        let routed = launch_of(LaunchRequest::Action { name: "show-anime".into(), id: None });
        assert_eq!(routed.error, Some(RouteError::MissingId("show-anime".into())));
        assert!(routed.actions.is_empty() && routed.ready);
    }

    #[test]
    fn an_unknown_action_is_an_error_and_does_not_set_ready() {
        let routed = launch_of(LaunchRequest::Action { name: "show-nothing".into(), id: None });
        assert_eq!(routed.error, Some(RouteError::UnknownAction("show-nothing".into())));
        assert!(!routed.ready);
    }

    #[test]
    fn a_search_pops_to_root_then_pushes_global_search_then_the_deep_link_page() {
        // T-36a-012: the order is PopToRoot, GlobalSearch, DeepLink
        let routed = launch_of(LaunchRequest::Search { query: "frieren".into(), kind: Kind::Anime });
        assert_eq!(
            routed.actions,
            vec![
                NavAction::PopToRoot,
                NavAction::Push(Screen::GlobalSearch { kind: Kind::Anime, query: "frieren".into(), filter: None }),
                NavAction::Push(Screen::DeepLink { kind: Kind::Anime, query: "frieren".into() }),
            ]
        );
        assert!(routed.ready);
    }

    #[test]
    fn an_empty_search_is_a_no_op_that_shows_the_window() {
        let routed = launch_of(LaunchRequest::Search { query: String::new(), kind: Kind::Manga });
        assert!(routed.actions.is_empty() && routed.ready);
    }

    #[test]
    fn a_search_of_the_manga_kind_uses_the_manga_global_search() {
        let routed = launch_of(LaunchRequest::Search { query: "berserk".into(), kind: Kind::Manga });
        assert!(matches!(&routed.actions[1], NavAction::Push(Screen::GlobalSearch { kind: Kind::Manga, .. })));
    }

    #[test]
    fn an_extension_search_pushes_global_search_with_its_filter_and_an_empty_one_does_nothing() {
        // T-36a-013
        let routed = route_request(&Intent::ExtSearch { kind: Kind::Manga, query: "x".into(), filter: Some("f".into()) });
        assert_eq!(
            routed.actions,
            vec![NavAction::PopToRoot, NavAction::Push(Screen::GlobalSearch { kind: Kind::Manga, query: "x".into(), filter: Some("f".into()) })]
        );
        let anime = route_request(&Intent::ExtSearch { kind: Kind::Anime, query: "y".into(), filter: None });
        assert!(matches!(&anime.actions[1], NavAction::Push(Screen::GlobalSearch { kind: Kind::Anime, .. })));
        let none = route_request(&Intent::ExtSearch { kind: Kind::Anime, query: String::new(), filter: None });
        assert!(none.actions.is_empty() && none.ready);
    }

    #[test]
    fn a_backup_file_pushes_the_restore_page_after_pop_to_root_and_another_file_is_ignored() {
        // T-36a-014
        assert_eq!(
            launch_of(LaunchRequest::OpenFile("/home/u/a.tachibk".into())).actions,
            vec![NavAction::PopToRoot, NavAction::Push(Screen::RestoreBackup("/home/u/a.tachibk".into()))]
        );
        let other = launch_of(LaunchRequest::OpenFile("/home/u/a.txt".into()));
        assert!(other.actions.is_empty() && other.ready);
    }

    #[test]
    fn repo_links_push_the_one_repo_list_and_a_missing_address_does_nothing() {
        // one repository list for both kinds: the tachiyomi and aniyomi add-repo links and the extension-store link all
        // open it with their address
        let manga = launch_of(LaunchRequest::OpenUri("tachiyomi://add-repo?url=https%3A%2F%2Fa.test%2Fi.json".into()));
        assert_eq!(
            manga.actions,
            vec![NavAction::PopToRoot, NavAction::Push(Screen::ExtensionStores { url: "https://a.test/i.json".into() })]
        );
        let anime = launch_of(LaunchRequest::OpenUri("aniyomi://extension-store?url=u".into()));
        assert!(matches!(&anime.actions[1], NavAction::Push(Screen::ExtensionStores { url }) if url == "u"));
        let missing = launch_of(LaunchRequest::OpenUri("aniyomi://add-repo".into()));
        assert!(missing.actions.is_empty() && missing.ready && missing.error.is_none());
    }

    #[test]
    fn an_unsupported_link_is_an_error_that_shows_the_window() {
        let routed = launch_of(LaunchRequest::OpenUri("ftp://x/y".into()));
        assert_eq!(routed.error, Some(RouteError::Uri(UriError::UnsupportedScheme("ftp".into()))));
        assert!(routed.ready);
    }

    #[test]
    fn an_invalid_request_from_the_argv_parser_is_its_error() {
        let routed = launch_of(LaunchRequest::Invalid(RouteError::UnknownKind("comic".into())));
        assert_eq!(routed.error, Some(RouteError::UnknownKind("comic".into())));
        assert!(routed.ready);
    }

    #[test]
    fn the_argv_parser_feeds_the_route_table() {
        // a command line goes through parse_argv and then routes to the same steps as the window expects
        let requests = argv::parse_argv(&argv(&["--search=frieren", "--type=manga"])).unwrap();
        let routed = route_request(&Intent::Launch(requests[0].clone()));
        assert_eq!(routed.actions.len(), 3);
    }
}
