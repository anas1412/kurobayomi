//! What a page hands to the browse screen it opens (T-40b-065, T-40b-038). `ui::open` takes a screen id and nothing else, so
//! the request is parked here first and the screen that builds takes it, once. The entry page parks its own genre and
//! title searches in `entry_core::handoff`; the screens read that as well (`search_screen`).

use std::cell::RefCell;

use super::route::BrowseRoute;

/// `eu.kanade.tachiyomi.SEARCH`: the extras `query` and `filter` (an extension package). An extension filter forces All and
/// limits the search to that package's sources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchIntent {
    pub query: String,
    pub extension: Option<String>,
}

thread_local! {
    static ROUTE: RefCell<Option<BrowseRoute>> = const { RefCell::new(None) };
    static INTENT: RefCell<Option<SearchIntent>> = const { RefCell::new(None) };
}

/// Parks `route` for the screen `ui::open` is about to build.
pub fn park(route: BrowseRoute) {
    ROUTE.with(|slot| *slot.borrow_mut() = Some(route));
}

/// The parked route, handed out once.
pub fn take() -> Option<BrowseRoute> {
    ROUTE.with(|slot| slot.borrow_mut().take())
}

/// The parked route when it is for `screen`; a route parked for another screen stays where it is.
pub fn take_for(screen: &str) -> Option<BrowseRoute> {
    ROUTE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.as_ref().is_some_and(|route| route.screen_id() == screen) { slot.take() } else { None }
    })
}

pub fn park_intent(intent: SearchIntent) {
    INTENT.with(|slot| *slot.borrow_mut() = Some(intent));
}

/// The parked search intent, handed out once.
pub fn take_intent() -> Option<SearchIntent> {
    INTENT.with(|slot| slot.borrow_mut().take())
}

/// Parks `route` and opens its screen.
pub fn open(route: BrowseRoute) {
    let screen = route.screen_id();
    park(route);
    crate::ui::open(screen);
}

/// The SEARCH intent: opens global search with the extension filter (T-40b-065). A blank query still opens the page.
pub fn open_search_intent(intent: SearchIntent) {
    park_intent(intent);
    crate::ui::open("global_search");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browse::route::BrowseKind;

    #[test]
    fn a_parked_route_is_handed_out_once_and_a_new_one_replaces_it() {
        assert_eq!(take(), None);
        park(BrowseRoute::MigrateEntries { kind: BrowseKind::Manga, source_id: 1 });
        park(BrowseRoute::GlobalSearch { kind: BrowseKind::Manga, source_id: None, query: "q".into() });
        assert_eq!(take(), Some(BrowseRoute::GlobalSearch { kind: BrowseKind::Manga, source_id: None, query: "q".into() }));
        assert_eq!(take(), None);
    }

    #[test]
    fn a_route_is_handed_out_only_to_the_screen_it_was_parked_for() {
        park(BrowseRoute::BrowseSource { source_id: 3, query: kuro_domain::source::browse::BrowseQuery::Popular, migrating: false });
        assert_eq!(take_for("global_search"), None, "another screen finds nothing");
        assert!(take_for("browse_source").is_some());
        assert_eq!(take_for("browse_source"), None, "and only once");
    }

    #[test]
    fn a_parked_intent_is_handed_out_once() {
        park_intent(SearchIntent { query: "one".into(), extension: Some("eu.a".into()) });
        assert_eq!(take_intent(), Some(SearchIntent { query: "one".into(), extension: Some("eu.a".into()) }));
        assert_eq!(take_intent(), None);
    }
}
