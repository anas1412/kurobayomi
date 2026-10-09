//! Reader UI state (T-043a-075, T-043a-076, and the menu, fullscreen and escape rules of T-043a-050). Pure: the
//! GTK side reads these values and applies them. Every change computes the new value first, then assigns it.

use super::model::ChapterState;

pub const BRIGHTNESS_MIN: i32 = -100;
pub const BRIGHTNESS_MAX: i32 = 100;

/// The one sheet or dialog the reader can show. `Loading` is the marker for an adjacent chapter load; it is
/// non-modal on Linux (the spinner is in the navigator), so it never hides another sheet (E-UI-5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialog {
    Loading,
    Settings,
    ReadingModeSelect,
    OrientationSelect,
    PageActions,
}

/// What Escape does: it dismisses an open sheet first, then leaves fullscreen, then closes the reader
/// (T-043a-050, T-043a-079).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscapeAction {
    DismissSheet,
    LeaveFullscreen,
    CloseReader,
}

/// The reader's visible state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiState {
    pub dialog: Option<Dialog>,
    pub menu_visible: bool,
    /// The software brightness overlay, kept in `-100..=100`.
    pub brightness: i32,
}

impl Default for UiState {
    fn default() -> Self {
        Self { dialog: None, menu_visible: true, brightness: 0 }
    }
}

impl UiState {
    /// Opens `dialog`, replacing whatever was open. The caller opens one sheet at a time.
    pub fn open(&mut self, dialog: Dialog) {
        self.dialog = Some(dialog);
    }

    /// Closes `dialog` only when it is the one open. Closing a different sheet changes nothing.
    pub fn close(&mut self, dialog: Dialog) {
        if self.dialog == Some(dialog) {
            self.dialog = None;
        }
    }

    /// An adjacent chapter load started: the loading marker shows only when no sheet is open.
    pub fn begin_adjacent_load(&mut self) {
        if self.dialog.is_none() {
            self.dialog = Some(Dialog::Loading);
        }
    }

    /// The adjacent load ended: only the loading marker is removed, never a sheet that is still open.
    pub fn end_adjacent_load(&mut self) {
        if self.dialog == Some(Dialog::Loading) {
            self.dialog = None;
        }
    }

    /// A tap or a key toggles the chrome. Returns the new visibility.
    pub fn toggle_menu(&mut self) -> bool {
        self.menu_visible = !self.menu_visible;
        self.menu_visible
    }

    pub fn set_menu_visible(&mut self, visible: bool) {
        self.menu_visible = visible;
    }

    /// The brightness overlay value, clamped to its range.
    pub fn set_brightness(&mut self, value: i32) {
        self.brightness = clamp_brightness(value);
    }
}

/// Clamps a brightness value to `-100..=100`.
pub fn clamp_brightness(value: i32) -> i32 {
    value.clamp(BRIGHTNESS_MIN, BRIGHTNESS_MAX)
}

/// Whether the window should be fullscreen for a menu state: hidden menu plus the fullscreen preference.
pub fn fullscreen_for_menu(menu_visible: bool, fullscreen_pref: bool) -> bool {
    fullscreen_pref && !menu_visible
}

/// F11 flips the window's fullscreen state.
pub fn toggle_fullscreen(current: bool) -> bool {
    !current
}

/// Escape dismisses an open sheet, else leaves fullscreen when the window is fullscreen, and closes the reader
/// otherwise. The sheet comes first wherever the focus is, so a sheet opened from the bottom bar is dismissed by
/// Escape and never closes the reader.
pub fn escape_action(sheet_open: bool, window_fullscreen: bool) -> EscapeAction {
    if sheet_open {
        EscapeAction::DismissSheet
    } else if window_fullscreen {
        EscapeAction::LeaveFullscreen
    } else {
        EscapeAction::CloseReader
    }
}

/// What the page area shows for a chapter state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadView {
    Spinner,
    Pages,
    Card,
}

/// Initial load and chapter states: a wait or a load shows the spinner, a loaded chapter shows its pages, and a
/// failure shows the error card with Retry (T-043a-024, T-043a-076).
pub fn load_view(state: &ChapterState) -> LoadView {
    match state {
        ChapterState::Wait | ChapterState::Loading => LoadView::Spinner,
        ChapterState::Loaded(_) => LoadView::Pages,
        ChapterState::Error(_) => LoadView::Card,
    }
}

/// A background load over a viewer that is already showing pages puts a spinner over the page area.
pub fn page_area_spinner(state: &ChapterState, viewer_showing: bool) -> bool {
    viewer_showing && matches!(state, ChapterState::Loading)
}

/// The chapter buttons are disabled while an adjacent chapter loads (T-043a-076).
pub fn navigator_buttons_enabled(adjacent_loading: bool) -> bool {
    !adjacent_loading
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::model::LoadError;
    use std::rc::Rc;

    #[test]
    fn a_sheet_is_closed_only_by_its_own_close() {
        let mut s = UiState::default();
        s.open(Dialog::PageActions);
        s.close(Dialog::Settings);
        assert_eq!(s.dialog, Some(Dialog::PageActions));
        s.close(Dialog::PageActions);
        assert_eq!(s.dialog, None);
    }

    #[test]
    fn closing_an_adjacent_load_keeps_the_sheet_whose_page_is_still_loaded() {
        let mut s = UiState::default();
        s.open(Dialog::PageActions);
        s.begin_adjacent_load();
        assert_eq!(s.dialog, Some(Dialog::PageActions), "the load marker never hides a sheet");
        s.end_adjacent_load();
        assert_eq!(s.dialog, Some(Dialog::PageActions));
    }

    #[test]
    fn the_load_marker_comes_and_goes_with_the_load() {
        let mut s = UiState::default();
        s.begin_adjacent_load();
        assert_eq!(s.dialog, Some(Dialog::Loading));
        s.end_adjacent_load();
        assert_eq!(s.dialog, None);
    }

    #[test]
    fn brightness_stays_in_range() {
        let mut s = UiState::default();
        s.set_brightness(-250);
        assert_eq!(s.brightness, -100);
        s.set_brightness(250);
        assert_eq!(s.brightness, 100);
        assert_eq!(clamp_brightness(-75), -75);
    }

    #[test]
    fn menu_toggles_and_reports_the_new_visibility() {
        let mut s = UiState::default();
        assert!(!s.toggle_menu());
        assert!(s.toggle_menu());
    }

    #[test]
    fn fullscreen_follows_the_menu_only_with_the_preference_on() {
        assert!(fullscreen_for_menu(false, true));
        assert!(!fullscreen_for_menu(true, true));
        assert!(!fullscreen_for_menu(false, false));
        assert!(toggle_fullscreen(false));
    }

    #[test]
    fn escape_dismisses_a_sheet_then_leaves_fullscreen_then_closes() {
        assert_eq!(escape_action(true, true), EscapeAction::DismissSheet);
        assert_eq!(escape_action(true, false), EscapeAction::DismissSheet);
        assert_eq!(escape_action(false, true), EscapeAction::LeaveFullscreen);
        assert_eq!(escape_action(false, false), EscapeAction::CloseReader);
    }

    #[test]
    fn load_states_map_to_their_visuals() {
        let err = ChapterState::Error(Rc::new(LoadError::new("offline")));
        assert_eq!(load_view(&ChapterState::Wait), LoadView::Spinner);
        assert_eq!(load_view(&ChapterState::Loading), LoadView::Spinner);
        assert_eq!(load_view(&err), LoadView::Card);
        assert!(page_area_spinner(&ChapterState::Loading, true));
        assert!(!page_area_spinner(&ChapterState::Loading, false));
        assert!(!navigator_buttons_enabled(true));
    }
}
