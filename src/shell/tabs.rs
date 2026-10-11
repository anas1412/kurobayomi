//! Tab host rules (Kotlin `HomeScreen`): the start tab, the back rule, bar visibility and reselect.
//!
//! The tab kinds, `NavStyle` and `StartScreen` come from `kuro_prefs::ui`; this module only holds the rules.

use std::cell::RefCell;
use std::rc::Rc;

use kuro_prefs::ui::{NavStyle, StartScreen, Tab};

/// The id of the window's tab page for a tab. Both libraries are one page with an Anime | Manga switcher.
pub fn nav_id(tab: Tab) -> &'static str {
    match tab {
        Tab::AnimeLibrary | Tab::MangaLibrary => "library",
        Tab::Updates => "updates",
        Tab::History => "history",
        Tab::Browse => "browse",
        Tab::More => "more",
    }
}

/// The tab a page id stands for. `current` decides between the two libraries, so an id that still names the selected
/// library keeps it.
pub fn tab_for_id(id: &str, current: Tab) -> Option<Tab> {
    Some(match id {
        "library" if current == Tab::MangaLibrary => Tab::MangaLibrary,
        "library" => Tab::AnimeLibrary,
        "updates" => Tab::Updates,
        "history" => Tab::History,
        "browse" => Tab::Browse,
        "more" => Tab::More,
        _ => return None,
    })
}

/// Hosts `home` on the tab pages in `stack`: the stack shows the selected tab now, and every later change of page is
/// recorded in `home`. One `HomeTabs` serves every layout, so a switch between the sidebar and the bottom bar keeps
/// the selection (T-36a-004).
pub fn bind(stack: &gtk::Stack, home: &Rc<RefCell<HomeTabs>>) {
    let wanted = nav_id(home.borrow().current());
    if stack.child_by_name(wanted).is_some() {
        stack.set_visible_child_name(wanted);
    }
    let home = home.clone();
    stack.connect_visible_child_name_notify(move |stack| {
        let Some(name) = stack.visible_child_name() else { return };
        let current = home.borrow().current();
        if let Some(tab) = tab_for_id(&name, current) {
            home.borrow_mut().select(tab);
        }
    });
}

/// Back or Escape on the root page: returns to the start tab when `home` handles back. Returns whether it did.
pub fn go_back(stack: &gtk::Stack, home: &Rc<RefCell<HomeTabs>>) -> bool {
    let handled = home.borrow_mut().back();
    if handled {
        stack.set_visible_child_name(nav_id(home.borrow().current()));
    }
    handled
}

/// The tab that "go to start" selects (HomeScreen `goToStartScreen`).
pub fn start_target(default: Tab, more: Tab) -> Tab {
    if default != more { default } else { Tab::AnimeLibrary }
}

/// The four-flag back rule `(A || B) && (C || D)` (behaviour section 2.6).
pub fn back_rule(a: bool, b: bool, c: bool, d: bool) -> bool {
    (a || b) && (c || d)
}

/// Whether the shell handles back: `enabled` in HomeScreen's `BackHandler`.
pub fn back_enabled(current: Tab, default: Tab, more: Tab) -> bool {
    back_rule(current == more, current != default, current != Tab::AnimeLibrary, default != more)
}

/// The bottom bar hides on the tab that was moved into More (behaviour section 2.3).
pub fn bar_visible(current: Tab, more: Tab, requested: bool) -> bool {
    requested && current != more
}

/// What a click on a tab does (behaviour section 2.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reselect {
    /// Selecting a tab that is already selected does nothing for most tabs.
    Nothing,
    /// The More tab pushes Settings.
    OpenSettings,
}

pub fn on_reselect(tab: Tab) -> Reselect {
    if tab == Tab::More { Reselect::OpenSettings } else { Reselect::Nothing }
}

/// The selected tab of the home screen. It is kept in memory across layout changes (`HomeTabs` key).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HomeTabs {
    current: Tab,
    default: Tab,
    more: Tab,
}

impl HomeTabs {
    /// Reads the start tab and the More tab. Built once per Home build, so a changed start screen applies there.
    pub fn new(start: StartScreen, style: NavStyle) -> Self {
        let default = start.tab();
        Self { current: default, default, more: style.more_tab() }
    }

    pub fn current(&self) -> Tab {
        self.current
    }

    pub fn more_tab(&self) -> Tab {
        self.more
    }

    pub fn select(&mut self, tab: Tab) {
        self.current = tab;
    }

    pub fn back_enabled(&self) -> bool {
        back_enabled(self.current, self.default, self.more)
    }

    /// Back or Escape: returns to the start tab when back is enabled. Returns whether it did.
    pub fn back(&mut self) -> bool {
        if !self.back_enabled() {
            return false;
        }
        self.current = start_target(self.default, self.more);
        true
    }

    #[cfg(test)]
    /// The bar entries shown for `style`, in order.
    pub fn bar_tabs(style: NavStyle) -> Vec<Tab> {
        style.tabs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Tab; 6] = [Tab::AnimeLibrary, Tab::MangaLibrary, Tab::Updates, Tab::History, Tab::Browse, Tab::More];

    #[test]
    fn start_target_uses_default_unless_it_is_the_more_tab() {
        assert_eq!(start_target(Tab::History, Tab::Browse), Tab::History);
        assert_eq!(start_target(Tab::History, Tab::History), Tab::AnimeLibrary);
    }

    #[test]
    fn back_rule_matches_the_kotlin_expression_for_all_sixteen_flag_sets() {
        for bits in 0u8..16 {
            let (a, b, c, d) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
            let kotlin = (a || b) && (c || d);
            assert_eq!(back_rule(a, b, c, d), kotlin, "flags a={a} b={b} c={c} d={d}");
        }
    }

    #[test]
    fn back_enabled_is_false_on_the_start_tab_and_true_elsewhere() {
        // default start is Anime, More is History: back is off on Anime
        assert!(!back_enabled(Tab::AnimeLibrary, Tab::AnimeLibrary, Tab::History));
        assert!(back_enabled(Tab::Browse, Tab::AnimeLibrary, Tab::History));
        // when the start tab is the More tab, the rule still returns from More to Anime
        assert!(back_enabled(Tab::History, Tab::History, Tab::History));
        assert!(!back_enabled(Tab::AnimeLibrary, Tab::History, Tab::History));
    }

    #[test]
    fn bar_hides_only_on_the_more_tab_when_requested() {
        assert!(!bar_visible(Tab::History, Tab::History, true));
        assert!(bar_visible(Tab::Browse, Tab::History, true));
        assert!(!bar_visible(Tab::Browse, Tab::History, false));
    }

    #[test]
    fn reselect_on_more_opens_settings() {
        assert_eq!(on_reselect(Tab::More), Reselect::OpenSettings);
        assert_eq!(on_reselect(Tab::Updates), Reselect::Nothing);
    }

    #[test]
    fn home_tabs_keep_the_selection_and_go_back_to_start() {
        let mut tabs = HomeTabs::new(StartScreen::MANGA, NavStyle::MOVE_HISTORY_TO_MORE);
        assert_eq!(tabs.current(), Tab::MangaLibrary);
        tabs.select(Tab::Browse);
        assert!(tabs.back());
        assert_eq!(tabs.current(), Tab::MangaLibrary);
        assert!(!tabs.back(), "back on the start tab is not handled");
    }

    #[test]
    fn back_from_a_tab_when_the_start_tab_is_more_goes_to_anime() {
        let mut tabs = HomeTabs::new(StartScreen::HISTORY, NavStyle::MOVE_HISTORY_TO_MORE);
        assert_eq!(tabs.more_tab(), Tab::History);
        tabs.select(Tab::Updates);
        assert!(tabs.back());
        assert_eq!(tabs.current(), Tab::AnimeLibrary);
    }

    #[test]
    fn bar_tabs_have_the_exact_order_for_each_style() {
        use Tab::*;
        assert_eq!(HomeTabs::bar_tabs(NavStyle::MOVE_MANGA_TO_MORE), vec![AnimeLibrary, Updates, History, Browse, More]);
        assert_eq!(HomeTabs::bar_tabs(NavStyle::MOVE_UPDATES_TO_MORE), vec![AnimeLibrary, MangaLibrary, History, Browse, More]);
        assert_eq!(HomeTabs::bar_tabs(NavStyle::MOVE_HISTORY_TO_MORE), vec![AnimeLibrary, MangaLibrary, Updates, Browse, More]);
        assert_eq!(HomeTabs::bar_tabs(NavStyle::MOVE_BROWSE_TO_MORE), vec![AnimeLibrary, MangaLibrary, Updates, History, More]);
    }

    #[test]
    fn every_tab_has_a_page_in_the_spec_and_ids_map_back() {
        let ids: Vec<String> = crate::ui::with_spec(|s| s.nav.iter().map(|n| n.id.clone()).collect());
        for tab in ALL {
            let id = nav_id(tab);
            assert!(ids.iter().any(|i| i == id), "{tab:?} has no page {id}");
            assert_eq!(tab_for_id(id, tab), Some(tab), "{tab:?}");
        }
        assert_eq!(tab_for_id("library", Tab::Updates), Some(Tab::AnimeLibrary));
        assert_eq!(tab_for_id("settings", Tab::Updates), None);
    }

    #[test]
    fn every_nav_style_lists_five_bar_tabs_ending_with_more() {
        for style in [NavStyle::MOVE_MANGA_TO_MORE, NavStyle::MOVE_UPDATES_TO_MORE, NavStyle::MOVE_HISTORY_TO_MORE, NavStyle::MOVE_BROWSE_TO_MORE] {
            let bar = HomeTabs::bar_tabs(style);
            assert_eq!(bar.len(), 5, "{style:?}");
            assert_eq!(bar.last(), Some(&Tab::More), "{style:?}");
            assert!(!bar.contains(&style.more_tab()), "{style:?}");
            assert!(ALL.iter().filter(|t| **t != style.more_tab()).all(|t| bar.contains(t)), "{style:?}");
        }
    }
}
