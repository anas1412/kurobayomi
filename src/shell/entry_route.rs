//! Entry open routing from the tabs (Kotlin `HomeScreen.openTab`, T-36a-060): a tab event selects its tab, and a tab with a
//! payload pushes the page that payload names.
//!
//! `effects` is pure and lists the steps in order. `open_tab` applies them to the tab selection and the navigator.

use kuro_prefs::ui::Tab;

use super::nav::{Kind, Navigator, Route};
use super::tabs::HomeTabs;

/// A tab event with its payload (Kotlin `HomeScreen.Tab`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenTab {
    /// The anime library, optionally opening one anime.
    AnimeLib { anime_id: Option<i64> },
    /// The manga library, optionally opening one manga.
    Library { manga_id: Option<i64> },
    Updates,
    History,
    /// The browse tab, optionally on the extensions page of the anime or manga catalogue.
    Browse { to_extensions: bool, anime: bool },
    /// The More tab, optionally on the downloads page.
    More { to_downloads: bool },
}

/// One step of an open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Select this tab.
    Select(Tab),
    /// Push this route onto the navigator.
    Push(Route),
    /// Show the extensions page of the anime (`true`) or manga (`false`) catalogue before the browse tab is selected.
    BrowseExtensions { anime: bool },
}

/// The steps for an open, in the order HomeScreen runs them: the tab first, then the page it names.
pub fn effects(open: OpenTab) -> Vec<Effect> {
    match open {
        OpenTab::AnimeLib { anime_id } => {
            let mut out = vec![Effect::Select(Tab::AnimeLibrary)];
            if let Some(id) = anime_id {
                out.push(Effect::Push(Route::Entry { kind: Kind::Anime, id, from_source: false }));
            }
            out
        }
        OpenTab::Library { manga_id } => {
            let mut out = vec![Effect::Select(Tab::MangaLibrary)];
            if let Some(id) = manga_id {
                out.push(Effect::Push(Route::Entry { kind: Kind::Manga, id, from_source: false }));
            }
            out
        }
        OpenTab::Updates => vec![Effect::Select(Tab::Updates)],
        OpenTab::History => vec![Effect::Select(Tab::History)],
        OpenTab::Browse { to_extensions, anime } => {
            let mut out = Vec::new();
            if to_extensions {
                out.push(Effect::BrowseExtensions { anime });
            }
            out.push(Effect::Select(Tab::Browse));
            out
        }
        OpenTab::More { to_downloads } => {
            let mut out = vec![Effect::Select(Tab::More)];
            if to_downloads {
                out.push(Effect::Push(Route::Downloads));
            }
            out
        }
    }
}

/// Applies an open: selects the tab, pushes the page, and hands the extensions request to `show_extensions`.
pub fn open_tab(open: OpenTab, home: &mut HomeTabs, nav: &mut Navigator, mut show_extensions: impl FnMut(bool)) {
    for effect in effects(open) {
        match effect {
            Effect::Select(tab) => home.select(tab),
            Effect::Push(route) => nav.push(route),
            Effect::BrowseExtensions { anime } => show_extensions(anime),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::ui::{NavStyle, StartScreen};

    #[test]
    fn an_anime_with_an_id_selects_the_anime_library_and_pushes_its_entry() {
        assert_eq!(
            effects(OpenTab::AnimeLib { anime_id: Some(7) }),
            vec![
                Effect::Select(Tab::AnimeLibrary),
                Effect::Push(Route::Entry { kind: Kind::Anime, id: 7, from_source: false }),
            ]
        );
        assert_eq!(effects(OpenTab::AnimeLib { anime_id: None }), vec![Effect::Select(Tab::AnimeLibrary)]);
    }

    #[test]
    fn a_manga_with_an_id_pushes_a_manga_entry() {
        assert_eq!(
            effects(OpenTab::Library { manga_id: Some(9) }),
            vec![
                Effect::Select(Tab::MangaLibrary),
                Effect::Push(Route::Entry { kind: Kind::Manga, id: 9, from_source: false }),
            ]
        );
    }

    #[test]
    fn the_downloads_request_pushes_the_downloads_page_from_more() {
        assert_eq!(effects(OpenTab::More { to_downloads: true }), vec![Effect::Select(Tab::More), Effect::Push(Route::Downloads)]);
        assert_eq!(effects(OpenTab::More { to_downloads: false }), vec![Effect::Select(Tab::More)]);
    }

    #[test]
    fn browse_asks_for_the_extensions_page_before_selecting_browse() {
        assert_eq!(
            effects(OpenTab::Browse { to_extensions: true, anime: true }),
            vec![Effect::BrowseExtensions { anime: true }, Effect::Select(Tab::Browse)]
        );
    }

    #[test]
    fn open_tab_selects_the_tab_and_pushes_the_entry_on_the_navigator() {
        let mut home = HomeTabs::new(StartScreen::ANIME, NavStyle::MOVE_HISTORY_TO_MORE);
        let mut nav = Navigator::new();
        let mut asked = None;
        open_tab(OpenTab::AnimeLib { anime_id: Some(3) }, &mut home, &mut nav, |anime| asked = Some(anime));
        assert_eq!(home.current(), Tab::AnimeLibrary);
        assert_eq!(nav.stack(), &[Route::Home, Route::Entry { kind: Kind::Anime, id: 3, from_source: false }]);
        assert_eq!(asked, None);

        open_tab(OpenTab::Browse { to_extensions: true, anime: false }, &mut home, &mut nav, |anime| asked = Some(anime));
        assert_eq!(asked, Some(false));
        assert_eq!(home.current(), Tab::Browse);
    }
}
