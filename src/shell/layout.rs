//! Layout chrome: sidebar or bottom bar, from the window width and the immersive rule (T-36a-005, T-36a-031, T-48a-007).
//!
//! Desktop decides by width only; the stored tablet UI mode has no effect.

use gtk::prelude::*;

/// Width from which the window is wide in Automatic mode (`TABLET_UI_REQUIRED_SCREEN_WIDTH_DP`, table TC-53: 719 is
/// compact, 720 is wide).
pub const WIDE_MIN_WIDTH_PX: i32 = 720;

/// The smallest window the app can be shrunk to (table TC-53, the phone size of the layout rules).
pub const MIN_WINDOW_SIZE: (i32, i32) = (360, 480);

/// Which tab chrome the window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chrome {
    Sidebar,
    BottomBar,
}

/// The chrome for a window `width` pixels wide. `maximized` counts as wide.
pub fn chrome_for(width: i32, maximized: bool) -> Chrome {
    if width >= WIDE_MIN_WIDTH_PX || maximized { Chrome::Sidebar } else { Chrome::BottomBar }
}

/// Sets the smallest size the window can be shrunk to (T-52a-044: 360 x 480).
pub fn apply_minimum_size(win: &impl IsA<gtk::Widget>) {
    win.set_size_request(MIN_WINDOW_SIZE.0, MIN_WINDOW_SIZE.1);
}

/// Which chrome elements are on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visibility {
    pub sidebar: bool,
    pub bottom_bar: bool,
}

/// Visibility of both chrome elements. The reader and the player are immersive: both hide. The bottom bar also hides
/// on the tab moved into More (`bar_allowed`, from `tabs::bar_visible`).
pub fn visibility(chrome: Chrome, immersive: bool, bar_allowed: bool) -> Visibility {
    if immersive {
        return Visibility { sidebar: false, bottom_bar: false };
    }
    match chrome {
        Chrome::Sidebar => Visibility { sidebar: true, bottom_bar: false },
        Chrome::BottomBar => Visibility { sidebar: false, bottom_bar: bar_allowed },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_720_px_breakpoint_decides_the_chrome() {
        // TC-53: 719 px is compact, 720 px is wide
        assert_eq!(chrome_for(500, false), Chrome::BottomBar);
        assert_eq!(chrome_for(719, false), Chrome::BottomBar);
        assert_eq!(chrome_for(720, false), Chrome::Sidebar);
        assert_eq!(chrome_for(900, false), Chrome::Sidebar);
    }

    #[test]
    fn the_minimum_window_is_360_by_480() {
        assert_eq!(MIN_WINDOW_SIZE, (360, 480));
    }

    #[test]
    fn a_maximized_window_is_wide() {
        assert_eq!(chrome_for(500, true), Chrome::Sidebar);
    }

    #[test]
    fn immersive_hides_both_chrome_elements() {
        for chrome in [Chrome::Sidebar, Chrome::BottomBar] {
            assert_eq!(visibility(chrome, true, true), Visibility { sidebar: false, bottom_bar: false });
        }
    }

    #[test]
    fn bottom_bar_hides_on_the_more_tab_and_the_sidebar_stays() {
        assert_eq!(visibility(Chrome::BottomBar, false, false), Visibility { sidebar: false, bottom_bar: false });
        assert_eq!(visibility(Chrome::BottomBar, false, true), Visibility { sidebar: false, bottom_bar: true });
        assert_eq!(visibility(Chrome::Sidebar, false, false), Visibility { sidebar: true, bottom_bar: false });
    }
}
