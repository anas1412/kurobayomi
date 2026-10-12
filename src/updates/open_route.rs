//! Which player opens a unit from the Updates feed (T-42b-018, T-42a-035, behaviour 12.5). The anime row tap uses the
//! player preference as it is (`alt` is false); the bar's play actions pass `alt` and so open the other player. The
//! internal player is `screens::open_unit`. The external launcher belongs to the player area; it registers with
//! [`install_external`], and until it does an external route falls back to the internal player.

use kuro_platform::kind::Kind;
use std::cell::RefCell;
use std::rc::Rc;

use crate::updates::bottom_bar::play_opens_external;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Internal,
    External,
}

/// The route of an open: `always_external != alt` for anime, the reader (internal) for manga.
pub fn route(kind: Kind, always_external: bool, alt: bool) -> Route {
    if kind.is_anime() && play_opens_external(always_external, alt) { Route::External } else { Route::Internal }
}

/// What actually opens: an external route needs a launcher.
pub fn resolve(route: Route, launcher_installed: bool) -> Route {
    if route == Route::External && launcher_installed { Route::External } else { Route::Internal }
}

/// Takes the entry id and the unit (episode) id.
type Launcher = Rc<dyn Fn(i64, i64) -> bool>;

thread_local! {
    static EXTERNAL: RefCell<Option<Launcher>> = const { RefCell::new(None) };
}

/// Registers the external player launcher: it gets the entry id and the unit id and says whether it launched.
pub fn install_external(launcher: impl Fn(i64, i64) -> bool + 'static) {
    EXTERNAL.with(|l| *l.borrow_mut() = Some(Rc::new(launcher)));
}

/// Opens the unit by `route`. A launcher that declines, or none, leaves the unit to the internal player.
pub fn open(kind: Kind, entry_id: i64, unit_id: i64, route: Route) {
    let launcher = EXTERNAL.with(|l| l.borrow().clone());
    if resolve(route, launcher.is_some()) == Route::External && launcher.is_some_and(|launch| launch(entry_id, unit_id)) {
        return;
    }
    if route == Route::External {
        tracing::info!("no external player took the unit; opening it in the internal player");
    }
    crate::screens::open_unit(kind, entry_id, unit_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_row_tap_follows_the_preference_and_the_bar_inverts_it() {
        assert_eq!(route(Kind::Anime, false, false), Route::Internal, "tap, internal preferred");
        assert_eq!(route(Kind::Anime, true, false), Route::External, "tap, external preferred");
        assert_eq!(route(Kind::Anime, false, true), Route::External, "bar play, alt");
        assert_eq!(route(Kind::Anime, true, true), Route::Internal);
    }

    #[test]
    fn manga_always_opens_in_the_reader() {
        assert_eq!(route(Kind::Manga, true, false), Route::Internal);
        assert_eq!(route(Kind::Manga, false, true), Route::Internal);
    }

    #[test]
    fn an_external_route_without_a_launcher_is_internal() {
        assert_eq!(resolve(Route::External, false), Route::Internal);
        assert_eq!(resolve(Route::External, true), Route::External);
        assert_eq!(resolve(Route::Internal, true), Route::Internal);
    }

    /// The one hook behind the Library, History and Updates openers: the installed launcher gets an external anime
    /// unit and nothing else. With no app data the internal fallback opens nothing, so the calls are all it shows.
    #[test]
    fn the_installed_launcher_is_asked_for_external_anime_units_only() {
        let asked: Rc<RefCell<Vec<(i64, i64)>>> = Rc::default();
        let log = Rc::clone(&asked);
        install_external(move |entry, unit| {
            log.borrow_mut().push((entry, unit));
            true
        });
        open(Kind::Anime, 1, 10, route(Kind::Anime, true, false));
        open(Kind::Anime, 2, 20, route(Kind::Anime, false, false));
        open(Kind::Manga, 3, 30, route(Kind::Manga, true, false));
        assert_eq!(*asked.borrow(), [(1, 10)], "only the anime unit whose preference is external");
    }
}
