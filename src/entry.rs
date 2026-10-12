// Pages reached from the entry screens that have no module of their own: the settings fallthrough. The entry
// details, the tracking pages, Browse, global search and migration pages are live (`entry_core`, `browse`).
use crate::screens::Built;
use gtk::Box as GBox;

pub fn body(id: &str) -> Option<Built> {
    match id {
        "tracking_sheet" => crate::entry_core::tracking::page::sheet(),
        "tracker_search" => crate::entry_core::tracking::page::search(),
        "extension_repos" => crate::settings::body("settings_extension_repos"),
        _ => crate::settings::body(id),
    }
}

/// The page of an entry that does not exist: `entry_core::route::build` falls back to it when no entry was named.
// ponytail: delete this with that fallback (build() could call page_for(0) as the anime route does).
pub fn entry(_title: &str, _anime: bool) -> GBox {
    crate::entry_core::route::target(0);
    crate::entry_core::route::build()
}
