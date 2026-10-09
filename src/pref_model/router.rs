//! Settings router (T-47b-001) and back navigation (T-47b-003). Pure: which page a destination opens, and what one
//! back action does. Escape, Alt+Left and the mouse back button all call [`Shell::back`].

/// Destinations other areas open by id (`SettingsScreen.kt`). Unknown ids open the default page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    About = 0,
    DataAndStorage = 1,
    Tracking = 2,
}

impl Destination {
    pub fn from_id(id: i64) -> Option<Self> {
        match id {
            0 => Some(Self::About),
            1 => Some(Self::DataAndStorage),
            2 => Some(Self::Tracking),
            _ => None,
        }
    }

    /// The spec screen the destination shows.
    pub fn screen_id(self) -> &'static str {
        match self {
            Self::About => "settings_about",
            Self::DataAndStorage => "settings_data_storage",
            Self::Tracking => "settings_tracking",
        }
    }
}

/// The widest window, in pixels, that still shows one pane: from this width the main list and the page sit side by
/// side (T-47b-002).
pub const TWO_PANE_WIDTH: i32 = 720;

/// Two panes from 720 px wide; 719 is one pane.
pub fn two_pane(width: i32) -> bool {
    width >= TWO_PANE_WIDTH
}

/// A page of the settings shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// The settings main list (narrow layouts only).
    Main,
    /// The Appearance page (wide layouts' default).
    Appearance,
    Dest(Destination),
    /// A settings sub-screen opened on top of the shell's own page.
    Inner,
}

/// The page a bare open shows: the main list on narrow layouts, Appearance on wide ones.
pub fn default_page(wide: bool) -> Page {
    if wide { Page::Appearance } else { Page::Main }
}

/// What one back action did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Back {
    /// An inner page was popped; the previous page is showing.
    Popped,
    /// No inner page was left; the shell closes.
    Closed,
}

/// The page stack of the settings shell. Empty means the shell is closed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shell {
    pages: Vec<Page>,
}

impl Shell {
    /// Opens the shell. `None` shows the default page. A destination shows only that page: on narrow layouts no
    /// main list is pushed underneath it.
    pub fn open(&mut self, dest: Option<Destination>, wide: bool) {
        self.pages = vec![match dest {
            Some(d) => Page::Dest(d),
            None => default_page(wide),
        }];
    }

    /// Pushes an inner page on top of the current one.
    pub fn push(&mut self, page: Page) {
        self.pages.push(page);
    }

    /// Closes the shell without touching the pages above it.
    pub fn close(&mut self) {
        self.pages.clear();
    }

    /// Makes the stack hold the shell's own page and exactly `inner` pages above it. The shell's pages are screens
    /// that other code opens and closes too, so the caller reads their count from the window's stack before it acts.
    pub fn sync(&mut self, inner: usize) {
        if self.pages.is_empty() {
            return;
        }
        self.pages.truncate(1 + inner);
        while self.pages.len() < 1 + inner {
            self.pages.push(Page::Inner);
        }
    }

    /// Pops the inner page if one exists, otherwise closes the shell.
    pub fn back(&mut self) -> Back {
        if self.pages.len() > 1 {
            self.pages.pop();
            Back::Popped
        } else {
            self.pages.clear();
            Back::Closed
        }
    }

    pub fn current(&self) -> Option<Page> {
        self.pages.last().copied()
    }

    pub fn is_open(&self) -> bool {
        !self.pages.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_map_to_the_three_destinations_and_unknown_ids_fall_back() {
        assert_eq!(Destination::from_id(0), Some(Destination::About));
        assert_eq!(Destination::from_id(1), Some(Destination::DataAndStorage));
        assert_eq!(Destination::from_id(2), Some(Destination::Tracking));
        assert_eq!(Destination::from_id(9), None);
        assert_eq!(Destination::from_id(-1), None);
    }

    #[test]
    fn bare_open_shows_the_default_page_for_the_layout() {
        let mut shell = Shell::default();
        shell.open(None, false);
        assert_eq!(shell.current(), Some(Page::Main));
        shell.open(None, true);
        assert_eq!(shell.current(), Some(Page::Appearance));
    }

    #[test]
    fn about_opens_directly_with_no_main_list_underneath() {
        let mut shell = Shell::default();
        shell.open(Some(Destination::About), false);
        assert_eq!(shell.current(), Some(Page::Dest(Destination::About)));
        assert_eq!(shell.back(), Back::Closed);
        assert!(!shell.is_open());
    }

    #[test]
    fn two_panes_start_at_720_px() {
        assert!(two_pane(720));
        assert!(!two_pane(719));
    }

    #[test]
    fn destinations_name_their_screens() {
        assert_eq!(Destination::About.screen_id(), "settings_about");
        assert_eq!(Destination::DataAndStorage.screen_id(), "settings_data_storage");
        assert_eq!(Destination::Tracking.screen_id(), "settings_tracking");
        crate::ui::with_spec(|spec| {
            for d in [Destination::About, Destination::DataAndStorage, Destination::Tracking] {
                assert!(spec.screen(d.screen_id()).is_some(), "{d:?}");
            }
        });
    }

    #[test]
    fn sync_follows_the_stack_the_window_holds() {
        let mut shell = Shell::default();
        shell.sync(2);
        assert!(!shell.is_open(), "a closed shell stays closed");
        shell.open(None, false);
        shell.sync(2);
        assert_eq!(shell.back(), Back::Popped);
        assert_eq!(shell.back(), Back::Popped);
        assert_eq!(shell.back(), Back::Closed);
        shell.open(None, false);
        shell.push(Page::Dest(Destination::About));
        shell.sync(0);
        assert_eq!(shell.current(), Some(Page::Main), "pages closed elsewhere are dropped");
    }

    #[test]
    fn back_pops_inner_pages_then_closes() {
        let mut shell = Shell::default();
        shell.open(None, false);
        shell.push(Page::Dest(Destination::Tracking));
        shell.push(Page::Dest(Destination::DataAndStorage));
        assert_eq!(shell.back(), Back::Popped);
        assert_eq!(shell.current(), Some(Page::Dest(Destination::Tracking)));
        assert_eq!(shell.back(), Back::Popped);
        assert_eq!(shell.current(), Some(Page::Main));
        assert_eq!(shell.back(), Back::Closed);
        assert!(!shell.is_open());
    }
}
