//! Sheet, panel and dialog host routing (area 46 `PlayerSheets.kt`, `PlayerPanels.kt`,
//! `PlayerDialogs.kt`; T-46a-027 and T-46a-028). Pure: the functions change `PlayerState` and return
//! what the window must still do. Layers stack as sheet, then panel, then dialog. Escape closes the
//! top one first.

use crate::enums::{Dialogs, Panels, Sheets};
use crate::overlay::ControlsModel;
use crate::state::PlayerState;

/// A layer of the overlay, from the top of the stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Sheet,
    Panel,
    Dialog,
}

/// Opens a sheet. A sheet replaces any other sheet, and closes the panel and dialog (VM:721-730).
/// The chapter sheet does nothing and closes when there is no chapter (E-46-065). Returns `true`
/// when a sheet opened, which means the controls must be hidden.
pub fn open_sheet(state: &mut PlayerState, sheet: Sheets, has_chapter: bool) -> bool {
    if sheet == Sheets::Chapters && !has_chapter {
        state.sheet = Sheets::None;
        return false;
    }
    state.sheet = sheet;
    if sheet == Sheets::None {
        return false;
    }
    state.panel = Panels::None;
    state.dialog = Dialogs::None;
    true
}

/// Opens a panel. A panel hides the controls and closes any sheet or dialog (VM:721-730). Returns
/// `true` when the controls must be hidden. `Panels::None` closes the panel and hides nothing.
pub fn open_panel(state: &mut PlayerState, panel: Panels) -> bool {
    if panel == Panels::None {
        state.panel = Panels::None;
        return false;
    }
    state.sheet = Sheets::None;
    state.dialog = Dialogs::None;
    state.panel = panel;
    true
}

/// Opens a dialog. `Dialogs::None` closes the current one.
pub fn open_dialog(state: &mut PlayerState, dialog: Dialogs) {
    state.dialog = dialog;
}

/// Whether any sheet, panel or dialog is open.
pub fn has_overlay(state: &PlayerState) -> bool {
    state.sheet != Sheets::None || state.panel != Panels::None || state.dialog != Dialogs::None
}

/// Escape or back, then the controls come back once nothing is open (E-46-076): `close_top`, and
/// `show` when no layer is left. The countdown restarts from `now_ms`.
pub fn close_top_and_show(
    state: &mut PlayerState,
    controls: &mut ControlsModel,
    now_ms: u64,
) -> Option<Layer> {
    let layer = close_top(state)?;
    if !has_overlay(state) {
        controls.show(now_ms, false);
    }
    Some(layer)
}

/// Closes the top layer and reports which one. `None` when nothing is open.
pub fn close_top(state: &mut PlayerState) -> Option<Layer> {
    if state.sheet != Sheets::None {
        state.sheet = Sheets::None;
        Some(Layer::Sheet)
    } else if state.panel != Panels::None {
        state.panel = Panels::None;
        Some(Layer::Panel)
    } else if state.dialog != Dialogs::None {
        state.dialog = Dialogs::None;
        Some(Layer::Dialog)
    } else {
        None
    }
}

/// Choosing a chapter closes the sheet. The caller seeks to the chapter start and then unpauses:
/// returns `true` for the unpause (PC:612-616).
pub fn choose_chapter(state: &mut PlayerState) -> bool {
    state.sheet = Sheets::None;
    true
}

/// Dismissing the screenshot sheet closes it and says whether to unpause. Only playback that was
/// running before the long press resumes (E-46-049).
pub fn dismiss_screenshot(state: &mut PlayerState, was_playing: bool) -> bool {
    state.sheet = Sheets::None;
    was_playing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_sheet_needs_a_chapter() {
        let mut s = PlayerState::default();
        open_sheet(&mut s, Sheets::Chapters, false);
        assert_eq!(s.sheet, Sheets::None);
        open_sheet(&mut s, Sheets::Chapters, true);
        assert_eq!(s.sheet, Sheets::Chapters);
    }

    #[test]
    fn opening_a_panel_closes_sheet_and_dialog_and_hides_controls() {
        let mut s = PlayerState::default();
        s.sheet = Sheets::More;
        s.dialog = Dialogs::EpisodeList;
        assert!(open_panel(&mut s, Panels::AudioDelay));
        assert_eq!(s.sheet, Sheets::None);
        assert_eq!(s.dialog, Dialogs::None);
        assert_eq!(s.panel, Panels::AudioDelay);
        assert!(!open_panel(&mut s, Panels::None));
        assert_eq!(s.panel, Panels::None);
    }

    #[test]
    fn escape_closes_sheet_then_panel_then_dialog() {
        let mut s = PlayerState::default();
        s.dialog = Dialogs::EpisodeList;
        s.panel = Panels::AudioDelay;
        s.sheet = Sheets::AudioTracks;
        assert!(has_overlay(&s));
        assert_eq!(close_top(&mut s), Some(Layer::Sheet));
        assert_eq!(close_top(&mut s), Some(Layer::Panel));
        assert!(has_overlay(&s));
        assert_eq!(close_top(&mut s), Some(Layer::Dialog));
        assert!(!has_overlay(&s));
        assert_eq!(close_top(&mut s), None);
    }

    #[test]
    fn integer_picker_dialog_routes_and_none_closes() {
        let mut s = PlayerState::default();
        open_dialog(
            &mut s,
            Dialogs::IntegerPicker {
                default_value: 1,
                min_value: 0,
                max_value: 5,
                step: 1,
                name_format: "%d".into(),
                title: "t".into(),
            },
        );
        assert!(has_overlay(&s));
        open_dialog(&mut s, Dialogs::None);
        assert!(!has_overlay(&s));
    }

    #[test]
    fn screenshot_dismissal_unpauses_only_when_it_was_playing() {
        let mut s = PlayerState::default();
        s.sheet = Sheets::Screenshot;
        assert!(!dismiss_screenshot(&mut s, false));
        assert_eq!(s.sheet, Sheets::None);
        s.sheet = Sheets::Screenshot;
        assert!(dismiss_screenshot(&mut s, true));
    }

    #[test]
    fn choosing_a_chapter_closes_the_sheet_and_asks_for_an_unpause() {
        let mut s = PlayerState::default();
        s.sheet = Sheets::Chapters;
        assert!(choose_chapter(&mut s));
        assert_eq!(s.sheet, Sheets::None);
    }

    #[test]
    fn opening_a_sheet_closes_the_panel_and_dialog_and_hides_controls() {
        let mut s = PlayerState::default();
        s.panel = Panels::VideoFilters;
        s.dialog = Dialogs::EpisodeList;
        assert!(open_sheet(&mut s, Sheets::More, true));
        assert_eq!(s.sheet, Sheets::More);
        assert_eq!(s.panel, Panels::None);
        assert_eq!(s.dialog, Dialogs::None);
        assert!(!open_sheet(&mut s, Sheets::None, true));
        assert_eq!(s.sheet, Sheets::None);
    }

    #[test]
    fn escape_shows_the_controls_only_after_the_last_layer_closes() {
        let mut s = PlayerState::default();
        let mut controls = ControlsModel::new(true, 4000);
        s.sheet = Sheets::More;
        s.panel = Panels::AudioDelay;
        assert_eq!(
            close_top_and_show(&mut s, &mut controls, 100),
            Some(Layer::Sheet)
        );
        assert!(!controls.shown());
        assert_eq!(
            close_top_and_show(&mut s, &mut controls, 200),
            Some(Layer::Panel)
        );
        assert!(controls.shown());
        assert_eq!(close_top_and_show(&mut s, &mut controls, 300), None);
    }
}
