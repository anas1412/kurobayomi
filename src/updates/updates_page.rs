//! Updates page behaviour (T-42a-034, T-42a-038): what Escape and the cover do in selection mode, which controls stay
//! enabled, what a finished read-state write leaves behind, and the page load state with its error row and retry.

use crate::updates::list::PageState;
use crate::updates::selection::{Selectable, Selection};

/// What the Back gesture or Escape does (EC-42-U-23): in selection mode it clears the selection and does not leave the
/// page; otherwise it navigates back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackAction {
    ClearSelection,
    Navigate,
}

pub fn back_action(selection_mode: bool) -> BackAction {
    if selection_mode { BackAction::ClearSelection } else { BackAction::Navigate }
}

/// Pull-to-refresh, the cover click and the download indicator are off in selection mode.
pub fn refresh_enabled(selection_mode: bool) -> bool {
    !selection_mode
}

pub fn cover_click_enabled(selection_mode: bool) -> bool {
    !selection_mode
}

/// After a read-state write: success clears the selection; a failure keeps it and returns the toast text (EC-42-U-11).
pub fn after_mark_read<T: Selectable>(
    result: Result<(), String>,
    selection: &mut Selection,
    items: &mut [T],
) -> Option<String> {
    match result {
        Ok(()) => {
            selection.toggle_all(items, false);
            None
        }
        Err(message) => Some(format!("Could not mark the updates: {message}")),
    }
}

/// The load state of the feed. A failed load stops the spinner and shows the error row with a retry (EC-42-U-03).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdatesLoad<T> {
    Loading,
    Loaded(Vec<T>),
    Failed(String),
}

impl<T> UpdatesLoad<T> {
    pub fn page(&self) -> PageState {
        match self {
            UpdatesLoad::Loading => PageState::Loading,
            UpdatesLoad::Loaded(rows) => PageState::for_load(true, rows.len(), &kuro_i18n::tr(kuro_i18n::MsgKey::information_no_recent)),
            UpdatesLoad::Failed(message) => PageState::Error(message.clone()),
        }
    }

    /// The retry of the error row starts the load again.
    pub fn retry(&mut self) {
        if matches!(self, UpdatesLoad::Failed(_)) {
            *self = UpdatesLoad::Loading;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct Row {
        id: i64,
        selected: bool,
    }

    impl Selectable for Row {
        fn id(&self) -> i64 {
            self.id
        }
        fn selected(&self) -> bool {
            self.selected
        }
        fn set_selected(&mut self, selected: bool) {
            self.selected = selected;
        }
    }

    #[test]
    fn updates_back_clears_selection_in_selection_mode() {
        assert_eq!(back_action(true), BackAction::ClearSelection);
        assert_eq!(back_action(false), BackAction::Navigate);
        assert!(!refresh_enabled(true));
        assert!(!cover_click_enabled(true));
        assert!(refresh_enabled(false));
    }

    #[test]
    fn updates_mark_read_success_clears_selection_and_failure_keeps_it() {
        let mut items = vec![Row { id: 1, selected: true }, Row { id: 2, selected: true }];
        let mut selection = Selection::default();
        assert_eq!(after_mark_read(Err("disk full".into()), &mut selection, &mut items), Some("Could not mark the updates: disk full".into()));
        assert!(items.iter().all(|r| r.selected));
        assert_eq!(after_mark_read(Ok(()), &mut selection, &mut items), None);
        assert!(items.iter().all(|r| !r.selected));
    }

    #[test]
    fn updates_error_is_not_a_stuck_loading_state() {
        let mut load: UpdatesLoad<i64> = UpdatesLoad::Loading;
        assert_eq!(load.page().child_name(), "loading");
        load = UpdatesLoad::Failed("query failed".into());
        assert_eq!(load.page(), PageState::Error("query failed".into()));
        load.retry();
        assert_eq!(load, UpdatesLoad::Loading);
        load = UpdatesLoad::Loaded(Vec::new());
        assert_eq!(load.page(), PageState::Empty("No recent updates".to_owned()));
    }
}
