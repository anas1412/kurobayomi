//! Updates page behaviour (T-42a-034, T-42a-038): what Escape and the cover do in selection mode, which controls stay
//! enabled, what a finished read-state write leaves behind, and the page load state with its error row and retry.

use crate::updates::copy;
use crate::updates::format;
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

    /// Applies a failed query (T-42b-003, EC-42-U-03). A feed already on screen stays and the subscription lives on; the
    /// returned text is the toast. With nothing on screen the error row with a retry takes the page.
    pub fn apply_failure(&mut self, error: &str) -> Option<String> {
        if matches!(self, UpdatesLoad::Loaded(_)) {
            return Some(copy::internal_error());
        }
        *self = UpdatesLoad::Failed(error.to_owned());
        None
    }
}

/// The row height of the feed in px (U 1.4).
pub const ROW_HEIGHT: i32 = 56;
/// The vertical inset of the cover in px, so the cover is `ROW_HEIGHT - 2 * COVER_INSET` tall.
pub const COVER_INSET: i32 = 6;
/// The cover width in px.
pub const COVER_WIDTH: i32 = 40;
/// The opacity of the title and the unit name of a read or seen row.
pub const READ_ALPHA: f64 = 0.38;

/// The opacity of a row's title and unit name.
pub fn row_alpha(done: bool) -> f64 {
    if done { READ_ALPHA } else { 1.0 }
}

/// The progress text of a row: only while unread and past the start (E-U10). Manga reads "Page: N" with N one-based
/// (E-U11). Anime reads "position/total" in the clock form of `progress_mmss`, a zero total included (E-U12, E-U13).
pub fn progress_text(anime: bool, done: bool, progress: i64, total_ms: i64) -> Option<String> {
    use kuro_i18n::{Arg, MsgKey, trf};
    if done || progress <= 0 {
        return None;
    }
    Some(if anime {
        trf(
            MsgKey::episode_progress,
            &[("arg1", Arg::Str(&format::progress_mmss(progress))), ("arg2", Arg::Str(&format::progress_mmss(total_ms)))],
        )
    } else {
        trf(MsgKey::chapter_progress, &[("arg1", Arg::Int(progress + 1))])
    })
}

/// The footer under a feed that hit the query limit (EC-42-U-02): the older rows are not shown.
pub fn limit_footer(rows: usize) -> Option<&'static str> {
    (rows == kuro_domain::UPDATES_LIMIT as usize).then_some(copy::SHOWING_LATEST_500)
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
    fn progress_only_when_unread_and_positive() {
        assert_eq!(progress_text(false, true, 4, 0), None, "a read row never shows progress");
        assert_eq!(progress_text(false, false, 0, 0), None);
        assert_eq!(progress_text(true, true, 5_000, 9_000), None);
        assert_eq!(progress_text(true, false, 0, 9_000), None);
    }

    #[test]
    fn manga_progress_is_one_based() {
        assert_eq!(progress_text(false, false, 4, 0).as_deref(), Some("Page: 5"));
        assert_eq!(progress_text(false, false, 1, 0).as_deref(), Some("Page: 2"));
    }

    #[test]
    fn format_progress_thresholds() {
        assert_eq!(progress_text(true, false, 3_600_000, 3_600_000).as_deref(), Some("60:00/60:00"));
        assert_eq!(progress_text(true, false, 3_600_001, 7_200_000).as_deref(), Some("1:00:00/2:00:00"));
    }

    #[test]
    fn anime_progress_with_zero_total() {
        assert_eq!(progress_text(true, false, 754_000, 0).as_deref(), Some("12:34/0:00"));
    }

    #[test]
    fn a_read_row_is_dimmed_and_the_row_geometry_is_the_spec() {
        assert_eq!(row_alpha(true), 0.38);
        assert_eq!(row_alpha(false), 1.0);
        assert_eq!(ROW_HEIGHT - 2 * COVER_INSET, 44);
        assert_eq!(COVER_WIDTH, 40);
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

    #[test]
    fn updates_failure_keeps_a_feed_on_screen_and_toasts() {
        let mut load = UpdatesLoad::Loaded(vec![1, 2]);
        assert_eq!(load.apply_failure("db locked"), Some(copy::internal_error()));
        assert_eq!(load, UpdatesLoad::Loaded(vec![1, 2]));
    }

    #[test]
    fn updates_failure_before_the_first_load_shows_the_error_row() {
        let mut load: UpdatesLoad<i64> = UpdatesLoad::Loading;
        assert_eq!(load.apply_failure("db locked"), None);
        assert_eq!(load.page(), PageState::Error("db locked".to_owned()));
    }

    #[test]
    fn updates_limit_footer_only_at_five_hundred() {
        assert_eq!(limit_footer(500), Some("Showing latest 500"));
        assert_eq!(limit_footer(499), None);
    }
}
