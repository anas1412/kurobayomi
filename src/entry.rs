// Pages reached from the entry screens that have no module of their own yet: the tracking sheet and the tracker
// search (an empty state until the tracking lane builds them), and the settings fallthrough. The entry
// details, Browse, global search and migration pages are live (`entry_core`, `browse`).
use crate::screens::Built;
use gtk::Box as GBox;

pub fn body(id: &str) -> Option<Built> {
    match id {
        // the tracking sheet and the tracker search wait for the tracking lane: an empty state, never made-up titles
        "tracking_sheet" | "tracker_search" => crate::screens::unavailable(id),
        "extension_repos" => crate::settings::body("settings_manga_extension_repos"),
        _ => crate::settings::body(id),
    }
}

/// The page of an entry that does not exist: `entry_core::route::build` falls back to it when no entry was named.
// ponytail: delete this with that fallback (build() could call page_for(0) as the anime route does).
pub fn entry(_title: &str, _anime: bool) -> GBox {
    crate::entry_core::route::target(0);
    crate::entry_core::route::build()
}
