//! App shell: the root navigator, the tab host, layout chrome, app-state banners, the app bar and search
//! toolbar, the onboarding wizard, launch bootstrap and argv parsing.
//!
//! Decisions and logic are pure Rust with unit tests. The GTK functions in each module are thin wrappers.

// the app bar, the search toolbar and the player and reader roots are built and tested here; the screens that
// call them live in other modules, so these four wait for their callers
#[allow(dead_code)]
pub mod appbar;
pub mod argv;
pub mod banners;
// the holds wait for the torrent and HTTP servers (T-53a-035), which start them from their own lanes
#[allow(dead_code)]
pub mod background;
// the splash timing and the migration gate wait for the startup lane; the downloaded filter waits for the reader's call
#[allow(dead_code)]
pub mod bootstrap;
#[allow(dead_code)]
pub mod downloaded;
// the route table and the share flow are called by the window's launch code, which applies their steps in its own lane
#[allow(dead_code)]
pub mod intents;
pub mod host;
pub mod launch;
pub mod layout;
// the stack-based navigator is replaced in the window by `ui::push`; it stays for the route table
#[allow(dead_code)]
pub mod nav;
pub mod onboarding;
// the installed desktop, MIME and icon files in `data/`; the module holds their tests
#[cfg(test)]
mod packaging;
#[allow(dead_code)]
pub mod player_root;
#[allow(dead_code)]
pub mod reader_root;
#[allow(dead_code)]
pub mod search;
pub mod tabs;
pub mod uri;

// the screens that use these build them in their own lanes; the rules are tested here
#[allow(dead_code)]
pub mod date_text;
// the resolvers (T-53a-048, T-53a-050) are installed with `deep_link_page::install_resolver` by the source lanes
#[allow(dead_code)]
pub mod deep_link_page;
#[allow(dead_code)]
pub mod empty;
#[allow(dead_code)]
pub mod entry_route;
#[allow(dead_code)]
pub mod lifecycle;
#[allow(dead_code)]
pub mod tabbed;
#[allow(dead_code)]
pub mod warning;
#[allow(dead_code)]
pub mod webview;

pub use host::Shell;
