//! What an entry page hands to the screens it opens (T-38a-016, T-38a-033, T-38a-017). `ui::open` takes a screen id and
//! nothing else, so a request that carries data (the genre to search, the entry to migrate, the page to show in the
//! WebView) is parked here first; the screen that opens takes it. A request is read once. No GTK here except the
//! `open_*` helpers, which push the screen after parking the request.

use std::cell::RefCell;

use gtk::prelude::WidgetExt;

use crate::entry_core::header::SearchRequest;
use crate::ui;

/// The page the WebView screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebViewRequest {
    pub url: String,
    pub title: String,
}

/// The entry a migrate flow starts from. `anime` picks the library the old entry lives in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrateRequest {
    pub entry_id: i64,
    pub anime: bool,
    /// The old entry's title, shown in the migrate dialog; a click on it opens the old entry (`open_old`).
    pub title: String,
}

impl MigrateRequest {
    /// The migrate dialog's click on the old title: opens the old entry's page.
    pub fn open_old(&self) {
        if self.anime {
            crate::entry_core::anime::route::open(self.entry_id);
        } else {
            crate::entry_core::route::open(self.entry_id);
        }
    }

    /// The migration is done: the pages above the old details page and the old page itself go, and the details page of
    /// the new entry takes their place (E-23).
    pub fn replace_old_page(&self, new_id: i64) {
        ui::pop_until(|page| matches!(page.widget_name().as_str(), "screen:manga_entry_details" | "screen:anime_entry_details"));
        ui::pop();
        if self.anime {
            crate::entry_core::anime::route::open(new_id);
        } else {
            crate::entry_core::route::open(new_id);
        }
    }
}

thread_local! {
    static SEARCH: RefCell<Option<SearchRequest>> = const { RefCell::new(None) };
    static WEBVIEW: RefCell<Option<WebViewRequest>> = const { RefCell::new(None) };
    static MIGRATE: RefCell<Option<MigrateRequest>> = const { RefCell::new(None) };
}

pub fn park_search(request: SearchRequest) {
    SEARCH.with(|s| *s.borrow_mut() = Some(request));
}

/// The search the opened screen should run, if a page parked one. It is handed out once.
pub fn take_search() -> Option<SearchRequest> {
    SEARCH.with(|s| s.borrow_mut().take())
}

pub fn park_webview(request: WebViewRequest) {
    WEBVIEW.with(|s| *s.borrow_mut() = Some(request));
}

pub fn take_webview() -> Option<WebViewRequest> {
    WEBVIEW.with(|s| s.borrow_mut().take())
}

pub fn park_migrate(request: MigrateRequest) {
    MIGRATE.with(|s| *s.borrow_mut() = Some(request));
}

pub fn take_migrate() -> Option<MigrateRequest> {
    MIGRATE.with(|s| s.borrow_mut().take())
}

/// A genre, author or title click: global for the text fields, the entry's own source for a genre or the source line.
pub fn open_search(request: SearchRequest) {
    let screen = if request.global { "global_search" } else { "browse_source" };
    park_search(request);
    ui::open(screen);
}

pub fn open_webview(request: WebViewRequest) {
    park_webview(request);
    ui::open("webview");
}

/// The overflow menu's Migrate: the search of the migrate flow, for this entry.
pub fn open_migrate(request: MigrateRequest) {
    park_migrate(request);
    ui::open("migrate_search");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_parked_search_is_handed_out_once_and_a_new_one_replaces_it() {
        assert_eq!(take_search(), None);
        park_search(SearchRequest { query: "Action".into(), global: false });
        park_search(SearchRequest { query: "Drama".into(), global: true });
        assert_eq!(take_search(), Some(SearchRequest { query: "Drama".into(), global: true }));
        assert_eq!(take_search(), None);
    }

    #[test]
    fn a_parked_migrate_request_carries_the_entry_and_the_kind() {
        park_migrate(MigrateRequest { entry_id: 7, anime: true, title: "Frieren".into() });
        let request = take_migrate().expect("parked");
        assert_eq!((request.entry_id, request.anime, request.title.as_str()), (7, true, "Frieren"));
        assert_eq!(take_migrate(), None);
    }

    #[test]
    fn a_parked_webview_request_is_handed_out_once() {
        park_webview(WebViewRequest { url: "https://example.org/a".into(), title: "A".into() });
        assert_eq!(take_webview().map(|r| r.url), Some("https://example.org/a".to_owned()));
        assert_eq!(take_webview(), None);
    }
}
