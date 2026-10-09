//! Dialog shell model (area 46 `PlayerDialog.kt`, T-46a-025). Pure: a modal window with a title and
//! an optional Cancel and OK row. The GTK shell draws it and sends each user action to `handle`.
//! Escape goes through `hosts::close_top`, so a dialog closes like any other layer.

use crate::enums::Dialogs;
use crate::hosts::{close_top, open_dialog};
use crate::state::PlayerState;

/// Padding around the content, in px.
pub const PADDING_PX: i32 = 16;
/// The window has no title bar or border.
pub const DECORATED: bool = false;
/// No width is set by the shell: the content decides it.
pub const DEFAULT_WIDTH: Option<i32> = None;

/// What the shell shows. A `None` label leaves that button out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogShell {
    pub title: String,
    pub cancel_label: Option<String>,
    pub ok_label: Option<String>,
}

impl DialogShell {
    /// Whether the button row is drawn at all.
    pub fn has_button_row(&self) -> bool {
        self.cancel_label.is_some() || self.ok_label.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogInput {
    Escape,
    /// A click on the dim area outside the dialog.
    OutsideClick,
    Cancel,
    Ok,
}

/// Applies one input. `Ok` runs `confirm` and then dismisses; every other input only dismisses.
/// Returns whether the dialog was dismissed (always true unless Escape found a sheet or panel
/// above it, which closes first).
pub fn handle(state: &mut PlayerState, input: DialogInput, confirm: impl FnOnce()) -> bool {
    match input {
        DialogInput::Escape => {
            close_top(state);
            state.dialog == Dialogs::None
        }
        DialogInput::OutsideClick | DialogInput::Cancel => {
            open_dialog(state, Dialogs::None);
            true
        }
        DialogInput::Ok => {
            confirm();
            open_dialog(state, Dialogs::None);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enums::Sheets;
    use std::cell::Cell;

    fn open() -> PlayerState {
        PlayerState {
            dialog: Dialogs::EpisodeList,
            ..PlayerState::default()
        }
    }

    #[test]
    fn ok_confirms_then_dismisses() {
        let mut state = open();
        let ran = Cell::new(false);
        let seen_open = Cell::new(false);
        assert!(handle(&mut state, DialogInput::Ok, || {
            ran.set(true);
            seen_open.set(true);
        }));
        assert!(ran.get() && seen_open.get());
        assert_eq!(state.dialog, Dialogs::None);
    }

    #[test]
    fn cancel_escape_and_outside_click_dismiss_without_confirming() {
        for input in [
            DialogInput::Cancel,
            DialogInput::Escape,
            DialogInput::OutsideClick,
        ] {
            let mut state = open();
            assert!(handle(&mut state, input, || panic!("must not confirm")));
            assert_eq!(state.dialog, Dialogs::None);
        }
    }

    #[test]
    fn escape_closes_a_sheet_above_the_dialog_first() {
        let mut state = open();
        state.sheet = Sheets::More;
        assert!(!handle(&mut state, DialogInput::Escape, || {}));
        assert_eq!(state.dialog, Dialogs::EpisodeList);
        assert!(handle(&mut state, DialogInput::Escape, || {}));
    }

    #[test]
    fn the_button_row_is_optional_and_the_shell_is_plain() {
        let shell = DialogShell {
            title: "Episodes".to_owned(),
            cancel_label: None,
            ok_label: None,
        };
        assert!(!shell.has_button_row());
        let with_ok = DialogShell {
            ok_label: Some("OK".to_owned()),
            ..shell
        };
        assert!(with_ok.has_button_row());
        assert_eq!(PADDING_PX, 16);
        assert!(!DECORATED);
        assert_eq!(DEFAULT_WIDTH, None);
    }
}
