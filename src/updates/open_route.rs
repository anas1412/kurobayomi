//! Which player opens a unit from the Updates feed (T-42b-018, T-42a-035, behaviour 12.5). The anime row tap uses the
//! player preference as it is (`alt` is false); the bar's play actions pass `alt` and so open the other player. The
//! internal player is `screens::open_media`. The external launcher belongs to the player area; it registers with
//! [`install_external`], and until it does an external route falls back to the internal player.

use std::cell::RefCell;
use std::rc::Rc;

use crate::updates::bottom_bar::play_opens_external;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Internal,
    External,
}

/// The route of an open: `always_external != alt` for anime, the reader (internal) for manga.
pub fn route(anime: bool, always_external: bool, alt: bool) -> Route {
    if anime && play_opens_external(always_external, alt) { Route::External } else { Route::Internal }
}

/// What actually opens: an external route needs a launcher.
pub fn resolve(route: Route, launcher_installed: bool) -> Route {
    if route == Route::External && launcher_installed { Route::External } else { Route::Internal }
}

type Launcher = Rc<dyn Fn(&str, &str) -> bool>;

thread_local! {
    static EXTERNAL: RefCell<Option<Launcher>> = const { RefCell::new(None) };
}

/// Registers the external player launcher: it gets the entry title and the unit name and says whether it launched.
pub fn install_external(launcher: impl Fn(&str, &str) -> bool + 'static) {
    EXTERNAL.with(|l| *l.borrow_mut() = Some(Rc::new(launcher)));
}

/// Opens the unit by `route`. A launcher that declines, or none, leaves the unit to the internal player.
pub fn open(anime: bool, title: &str, unit: &str, route: Route) {
    let launcher = EXTERNAL.with(|l| l.borrow().clone());
    if resolve(route, launcher.is_some()) == Route::External && launcher.is_some_and(|launch| launch(title, unit)) {
        return;
    }
    if route == Route::External {
        tracing::info!("no external player took the unit; opening it in the internal player");
    }
    crate::screens::open_media(anime, title, unit);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_row_tap_follows_the_preference_and_the_bar_inverts_it() {
        assert_eq!(route(true, false, false), Route::Internal, "tap, internal preferred");
        assert_eq!(route(true, true, false), Route::External, "tap, external preferred");
        assert_eq!(route(true, false, true), Route::External, "bar play, alt");
        assert_eq!(route(true, true, true), Route::Internal);
    }

    #[test]
    fn manga_always_opens_in_the_reader() {
        assert_eq!(route(false, true, false), Route::Internal);
        assert_eq!(route(false, false, true), Route::Internal);
    }

    #[test]
    fn an_external_route_without_a_launcher_is_internal() {
        assert_eq!(resolve(Route::External, false), Route::Internal);
        assert_eq!(resolve(Route::External, true), Route::External);
        assert_eq!(resolve(Route::Internal, true), Route::Internal);
    }
}
