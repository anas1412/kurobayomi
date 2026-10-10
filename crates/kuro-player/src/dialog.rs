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

/// The values of an integer picker: `min`, `min + step`, and so on up to `max` (`min..=max step`).
/// `None` when `step` is not positive, which Kotlin rejects (E-46-028b).
pub fn picker_values(min: i32, max: i32, step: i32) -> Option<Vec<i32>> {
    (step > 0).then(|| (min..=max).step_by(step as usize).collect())
}

/// The row the picker opens on. A default outside the range of `values` is clamped into it on open
/// (U§6, T-45b-080). A default inside the range but off the step has no row, so it opens on row 0
/// (E-46-028).
pub fn picker_start_index(values: &[i32], default: i32) -> usize {
    let (Some(&lo), Some(&hi)) = (values.first(), values.last()) else {
        return 0;
    };
    let value = default.clamp(lo, hi);
    values.iter().position(|v| *v == value).unwrap_or(0)
}

/// What a dismissal commits: the value of the displayed row, never the default (E-46-027).
pub fn commit_value(values: &[i32], selection: usize) -> Option<i32> {
    values.get(selection).copied()
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

#[cfg(test)]
mod picker_tests {
    use super::*;

    #[test]
    fn values_step_up_to_the_maximum_and_skip_past_it() {
        assert_eq!(picker_values(0, 10, 3), Some(vec![0, 3, 6, 9]));
        assert_eq!(picker_values(5, 5, 1), Some(vec![5]));
        assert_eq!(picker_values(10, 0, 1), Some(vec![]));
    }

    #[test]
    fn a_zero_or_negative_step_is_rejected() {
        assert_eq!(picker_values(0, 10, 0), None);
        assert_eq!(picker_values(0, 10, -2), None);
    }

    #[test]
    fn the_picker_starts_on_the_default_or_the_first_row() {
        let values = [0, 3, 6, 9];
        assert_eq!(picker_start_index(&values, 6), 2);
        assert_eq!(picker_start_index(&values, 7), 0);
    }

    #[test]
    fn an_out_of_range_default_is_clamped_on_open() {
        let values = [0, 3, 6, 9];
        assert_eq!(picker_start_index(&values, 50), 3);
        assert_eq!(picker_start_index(&values, -5), 0);
        assert_eq!(picker_start_index(&[], 4), 0);
    }

    #[test]
    fn the_commit_is_the_displayed_row() {
        let values = [0, 3, 6, 9];
        assert_eq!(commit_value(&values, 1), Some(3));
        assert_eq!(commit_value(&values, 9), None);
    }
}
