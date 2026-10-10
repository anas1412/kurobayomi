//! Layout chrome: sidebar or bottom bar, from the tablet UI mode, the window size and the immersive rule (T-36a-005, T-36a-031).
//!
//! Mode table: behaviour section 8.2. `TabletUiMode` comes from `kuro_prefs::ui`.

use gtk::prelude::*;
use kuro_prefs::ui::TabletUiMode;

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

/// The chrome for a window of `width` x `height` pixels. `maximized` counts as wide in Automatic mode.
pub fn chrome_for(mode: TabletUiMode, width: i32, height: i32, maximized: bool) -> Chrome {
    let wide = width >= WIDE_MIN_WIDTH_PX || maximized;
    let sidebar = match mode {
        TabletUiMode::AUTOMATIC => wide,
        TabletUiMode::ALWAYS => true,
        TabletUiMode::LANDSCAPE => width > height,
        TabletUiMode::NEVER => false,
    };
    if sidebar { Chrome::Sidebar } else { Chrome::BottomBar }
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

    const MODES: [TabletUiMode; 4] = [TabletUiMode::AUTOMATIC, TabletUiMode::ALWAYS, TabletUiMode::LANDSCAPE, TabletUiMode::NEVER];

    fn chrome(mode: TabletUiMode, width: i32, height: i32) -> Chrome {
        chrome_for(mode, width, height, false)
    }

    #[test]
    fn automatic_uses_the_720_px_breakpoint() {
        // TC-53: AUTO 719 px is compact, 720 px is wide
        assert_eq!(chrome(TabletUiMode::AUTOMATIC, 500, 800), Chrome::BottomBar);
        assert_eq!(chrome(TabletUiMode::AUTOMATIC, 719, 800), Chrome::BottomBar);
        assert_eq!(chrome(TabletUiMode::AUTOMATIC, 720, 800), Chrome::Sidebar);
        assert_eq!(chrome(TabletUiMode::AUTOMATIC, 900, 600), Chrome::Sidebar);
    }

    #[test]
    fn table_tc_53_rows() {
        // the rows of TC-53 in plans/52-util-app/p1-edge-cases-and-tests.md section 5.13
        assert_eq!(chrome(TabletUiMode::AUTOMATIC, 719, 800), Chrome::BottomBar);
        assert_eq!(chrome(TabletUiMode::AUTOMATIC, 720, 800), Chrome::Sidebar);
        assert_eq!(chrome(TabletUiMode::LANDSCAPE, 1000, 800), Chrome::Sidebar);
        assert_eq!(chrome(TabletUiMode::LANDSCAPE, 800, 800), Chrome::BottomBar, "a square window is not landscape");
        assert_eq!(chrome(TabletUiMode::ALWAYS, 300, 300), Chrome::Sidebar);
        assert_eq!(chrome(TabletUiMode::NEVER, 2000, 1000), Chrome::BottomBar);
    }

    #[test]
    fn the_minimum_window_is_360_by_480() {
        assert_eq!(MIN_WINDOW_SIZE, (360, 480));
    }

    #[test]
    fn automatic_treats_a_maximized_window_as_wide() {
        assert_eq!(chrome_for(TabletUiMode::AUTOMATIC, 500, 800, true), Chrome::Sidebar);
    }

    #[test]
    fn always_is_sidebar_at_every_width_and_orientation() {
        for (w, h) in [(500, 800), (500, 300), (720, 800), (900, 600), (900, 1200)] {
            assert_eq!(chrome(TabletUiMode::ALWAYS, w, h), Chrome::Sidebar, "{w}x{h}");
        }
    }

    #[test]
    fn landscape_follows_the_orientation_only() {
        assert_eq!(chrome(TabletUiMode::LANDSCAPE, 500, 800), Chrome::BottomBar, "portrait narrow");
        assert_eq!(chrome(TabletUiMode::LANDSCAPE, 760, 1000), Chrome::BottomBar, "portrait wide");
        assert_eq!(chrome(TabletUiMode::LANDSCAPE, 900, 600), Chrome::Sidebar, "landscape wide");
        assert_eq!(chrome(TabletUiMode::LANDSCAPE, 500, 300), Chrome::Sidebar, "landscape narrow");
    }

    #[test]
    fn never_is_bottom_bar_at_every_width_and_orientation() {
        for (w, h) in [(500, 800), (500, 300), (720, 800), (900, 600), (900, 1200)] {
            assert_eq!(chrome(TabletUiMode::NEVER, w, h), Chrome::BottomBar, "{w}x{h}");
        }
    }

    #[test]
    fn every_mode_is_checked_at_the_three_breakpoint_widths() {
        // widths 500, 720 and 900, portrait and landscape: the table has one answer per cell
        let expect = |mode: TabletUiMode, w: i32, landscape: bool| match mode {
            TabletUiMode::AUTOMATIC => if w >= WIDE_MIN_WIDTH_PX { Chrome::Sidebar } else { Chrome::BottomBar },
            TabletUiMode::ALWAYS => Chrome::Sidebar,
            TabletUiMode::LANDSCAPE => if landscape { Chrome::Sidebar } else { Chrome::BottomBar },
            TabletUiMode::NEVER => Chrome::BottomBar,
        };
        for mode in MODES {
            for w in [500, 720, 900] {
                for landscape in [false, true] {
                    let h = if landscape { w - 100 } else { w + 300 };
                    assert_eq!(chrome(mode, w, h), expect(mode, w, landscape), "{mode:?} {w}x{h}");
                }
            }
        }
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
