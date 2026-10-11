//! History screen logic (T-42a-013, T-42a-015, T-42a-016, T-42a-018, T-42a-023): the next-unit rule with its legacy
//! switch, the resume outcome, the view state of a list, the search match and the remove dialog copy. The queries and
//! the writes are in `kuro_domain::history`; this file decides what the screen does with their results.

use chrono::{NaiveDate, TimeZone};
use kuro_domain::dates::{ListRow, local_date, short_time};
use kuro_domain::history::model::HistoryWithRelations;
use kuro_domain::history::view::HistoryLine;
use kuro_domain::kind::EntryKind;
use kuro_i18n::{MsgKey, tr};

use crate::updates::dates::grouped;
use crate::updates::history_row::subtitle;

/// The toast of a resume that finds nothing stays for 4 seconds.
pub const NOT_FOUND_TOAST_MS: u64 = 4_000;

/// The toast after "Remove everything" succeeded stays for 4 seconds.
pub const CLEAR_DONE_TOAST_MS: u64 = 4_000;

/// A table change re-runs the query after this quiet time (bursts from one batch collapse into one run).
pub const COALESCE_MS: u64 = 50;

/// `GetNextChapters` / `GetNextEpisodes` with `onlyUnread = false`, over a list already in reading order.
///
/// The current unit (`from_id`) is returned first when it is unread, else the list starts at the following unit. When
/// `from_id` is not in the list the result starts at the first unit. With `legacy_second_unit_on_missing` the result
/// starts at the second unit instead, which is what Aniyomi returns (EC-42-H-05).
pub fn next_units_from<T: Clone>(
    ordered: &[T],
    id: impl Fn(&T) -> i64,
    done: impl Fn(&T) -> bool,
    from_id: i64,
    legacy_second_unit_on_missing: bool,
) -> Vec<T> {
    let start = match ordered.iter().position(|x| id(x) == from_id) {
        Some(i) if done(&ordered[i]) => i + 1,
        Some(i) => i,
        None if legacy_second_unit_on_missing => 1,
        None => 0,
    };
    ordered.get(start..).unwrap_or(&[]).to_vec()
}

/// What a resume does: open the first next unit, or show the not-found toast of the kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resume<T> {
    Open(T),
    NotFound { message: String, toast_ms: u64 },
}

pub fn resume<T>(next: Vec<T>, kind: EntryKind) -> Resume<T> {
    match next.into_iter().next() {
        Some(unit) => Resume::Open(unit),
        None => Resume::NotFound { message: not_found_message(kind), toast_ms: NOT_FOUND_TOAST_MS },
    }
}

/// The toast of a resume that finds no unit (`no_next_chapter`, `no_next_episode`).
pub fn not_found_message(kind: EntryKind) -> String {
    tr(match kind {
        EntryKind::Manga => MsgKey::no_next_chapter,
        EntryKind::Anime => MsgKey::no_next_episode,
    })
}

/// The text of an empty list: "No results found" while a search is active, else the kind's own text.
pub fn empty_message(kind: EntryKind, search_active: bool) -> String {
    tr(match (search_active, kind) {
        (true, _) => MsgKey::no_results_found,
        (false, EntryKind::Manga) => MsgKey::information_no_recent_manga,
        (false, EntryKind::Anime) => MsgKey::information_no_recent_anime,
    })
}

/// The snackbar after "Remove everything" was confirmed: "History deleted" on success, the internal-error text when
/// the write failed (the list is left as it was).
pub fn clear_outcome(cleared: bool) -> String {
    tr(if cleared { MsgKey::clear_history_completed } else { MsgKey::internal_error })
}

/// The dated lines of the list: a header per local day (the stored `app_date_format`, or the relative words) and the
/// row text `Ch. {n} - {time}` / `Ep. {n} - {time}`.
pub fn history_lines<Tz: TimeZone>(
    rows: &[HistoryWithRelations],
    kind: EntryKind,
    today: NaiveDate,
    zone: &Tz,
    relative: bool,
    date_pref: &str,
    locale: &str,
) -> Vec<ListRow<HistoryLine>> {
    let prefix = if kind == EntryKind::Manga { "Ch." } else { "Ep." };
    let lines = rows.iter().filter(|r| local_date(r.at, zone).is_some()).map(|r| {
        let time = short_time(r.at, zone).unwrap_or_default();
        let line = HistoryLine {
            history_id: r.id,
            item_id: r.item_id,
            entry_id: r.entry_id,
            title: r.title.clone(),
            text: subtitle(prefix, r.item_number, &time),
            cover: r.cover.clone(),
        };
        (line, r.at)
    });
    grouped(lines, zone, today, relative, date_pref, locale)
}

/// The dialog the history screen shows, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryDialog {
    /// Remove one row (and, with the checkbox, every row of the entry).
    Remove { history_id: i64, entry_id: i64 },
    /// "Remove everything" of the visible kind.
    ClearAll,
}

/// The state of one history list: `{ list, dialog, search }` plus the pending toast. `list == None` is the loading
/// state. A failed query keeps the last list and raises a toast (`set_error`), and a search change asks the caller to
/// cancel the previous query (`set_search` returns true).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryState<T> {
    pub list: Option<Vec<T>>,
    pub dialog: Option<HistoryDialog>,
    pub search: String,
    pub toast: Option<String>,
}

impl<T> Default for HistoryState<T> {
    fn default() -> Self {
        Self { list: None, dialog: None, search: String::new(), toast: None }
    }
}

impl<T> HistoryState<T> {
    #[cfg(test)]
    pub fn is_loading(&self) -> bool {
        self.list.is_none()
    }

    pub fn set_rows(&mut self, rows: Vec<T>) {
        self.list = Some(rows);
        self.toast = None;
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.toast = Some(message.into());
    }

    pub fn open_dialog(&mut self, dialog: HistoryDialog) {
        self.dialog = Some(dialog);
    }

    /// Closes the dialog and returns the one that was open.
    pub fn dismiss_dialog(&mut self) -> Option<HistoryDialog> {
        self.dialog.take()
    }

    /// True when the text changed, which means the query must run again and the previous run is cancelled.
    pub fn set_search(&mut self, text: &str) -> bool {
        if self.search == text {
            return false;
        }
        self.search = text.to_owned();
        true
    }
}

/// The copy of the remove dialog (T-42a-018). The checkbox resets every unit of the entry; unchecked resets one row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoveDialog {
    pub title: String,
    pub text: String,
    pub reset_label: String,
    pub confirm: String,
    pub cancel: String,
}

pub fn remove_dialog(kind: EntryKind) -> RemoveDialog {
    let (text, reset) = match kind {
        EntryKind::Manga => (MsgKey::dialog_with_checkbox_remove_description, MsgKey::dialog_with_checkbox_reset),
        EntryKind::Anime => {
            (MsgKey::dialog_with_checkbox_remove_description_anime, MsgKey::dialog_with_checkbox_reset_anime)
        }
    };
    RemoveDialog {
        title: tr(MsgKey::action_remove),
        text: tr(text),
        reset_label: tr(reset),
        confirm: tr(MsgKey::action_remove),
        cancel: tr(MsgKey::action_cancel),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (id, done) in reading order.
    fn units() -> Vec<(i64, bool)> {
        vec![(1, true), (2, true), (3, false), (4, false)]
    }

    fn next(from: i64, legacy: bool) -> Vec<i64> {
        next_units_from(&units(), |u| u.0, |u| u.1, from, legacy).into_iter().map(|u| u.0).collect()
    }

    #[test]
    fn history_next_units_unread_current_is_first() {
        assert_eq!(next(3, false), vec![3, 4]);
    }

    #[test]
    fn history_next_units_read_current_drops_it() {
        assert_eq!(next(2, false), vec![3, 4]);
    }

    #[test]
    fn history_next_units_missing_from_id_returns_first_unit() {
        assert_eq!(next(99, false), vec![1, 2, 3, 4]);
    }

    #[test]
    fn history_next_units_legacy_mode_starts_at_second_unit() {
        assert_eq!(next(99, true), vec![2, 3, 4]);
    }

    #[test]
    fn history_resume_opens_first_or_shows_not_found() {
        assert_eq!(resume(vec![7, 8], EntryKind::Manga), Resume::Open(7));
        assert_eq!(
            resume(Vec::<i64>::new(), EntryKind::Manga),
            Resume::NotFound { message: "Next chapter not found".to_owned(), toast_ms: 4_000 }
        );
        assert_eq!(
            resume(Vec::<i64>::new(), EntryKind::Anime),
            Resume::NotFound { message: "Next Episode not found!".to_owned(), toast_ms: 4_000 }
        );
    }

    #[test]
    fn history_error_keeps_last_list() {
        let mut state: HistoryState<i64> = HistoryState::default();
        assert!(state.is_loading());
        state.set_rows(vec![1, 2]);
        state.set_error("query failed");
        assert_eq!(state.list, Some(vec![1, 2]));
        assert_eq!(state.toast.as_deref(), Some("query failed"));
        state.set_rows(vec![3]);
        assert_eq!(state.toast, None);
    }

    #[test]
    fn history_search_change_asks_for_requery_only_when_text_changes() {
        let mut state: HistoryState<i64> = HistoryState::default();
        assert!(state.set_search("one"));
        assert!(!state.set_search("one"));
        assert!(state.set_search(""));
    }

    #[test]
    fn history_clear_all_reports_success_or_failure() {
        assert_eq!(clear_outcome(true), "History deleted");
        assert_eq!(clear_outcome(false), "InternalError: Check crash logs for further information");
    }

    #[test]
    fn history_remove_dialog_copy_is_kind_specific() {
        let manga = remove_dialog(EntryKind::Manga);
        assert_eq!(manga.title, "Remove");
        assert_eq!(manga.text, "This will remove the read date of this chapter. Are you sure?");
        assert_eq!(manga.reset_label, "Reset all chapters for this manga");
        assert_eq!((manga.confirm.as_str(), manga.cancel.as_str()), ("Remove", "Cancel"));
        assert_eq!(remove_dialog(EntryKind::Anime).text, "This will remove the watch date of this episode. Are you sure?");
    }

    #[test]
    fn history_state_holds_one_dialog_until_dismissed() {
        let mut state: HistoryState<i64> = HistoryState::default();
        assert_eq!(state.dialog, None);
        state.open_dialog(HistoryDialog::Remove { history_id: 3, entry_id: 9 });
        assert_eq!(state.dialog, Some(HistoryDialog::Remove { history_id: 3, entry_id: 9 }));
        assert_eq!(state.dismiss_dialog(), Some(HistoryDialog::Remove { history_id: 3, entry_id: 9 }));
        assert_eq!(state.dismiss_dialog(), None);
    }

    #[test]
    fn history_empty_text_depends_on_search_and_kind() {
        use kuro_domain::history::view::empty_text;
        for kind in [EntryKind::Manga, EntryKind::Anime] {
            for search in [false, true] {
                assert_eq!(empty_message(kind, search), empty_text(kind, search), "same words as the domain table");
            }
        }
        assert_eq!(empty_message(EntryKind::Manga, false), "Nothing read recently");
        assert_eq!(empty_message(EntryKind::Anime, false), "Nothing watched recently");
        assert_eq!(empty_message(EntryKind::Anime, true), "No results found");
    }

    #[test]
    fn history_lines_group_by_day_with_the_row_text() {
        use chrono::Utc;
        use kuro_domain::history::model::CoverKey;
        let row = |id: i64, at: i64, number: f64| HistoryWithRelations {
            id,
            item_id: id * 10,
            entry_id: 1,
            title: "T".into(),
            item_number: number,
            at,
            duration_ms: 0,
            cover: CoverKey::create(),
        };
        let day = |d: u32, h: u32| Utc.with_ymd_and_hms(2026, 10, d, h, 5, 0).unwrap().timestamp_millis();
        let today = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
        let lines = history_lines(&[row(1, day(9, 14), 12.0), row(2, day(8, 9), -1.0)], EntryKind::Manga, today, &Utc, true, "", "en");
        let texts: Vec<String> = lines
            .iter()
            .map(|l| match l {
                ListRow::Header(h) => format!("# {h}"),
                ListRow::Item(i) => i.text.clone(),
            })
            .collect();
        assert_eq!(texts, ["# Today", "Ch. 12 - 14:05", "# Yesterday", "09:05"]);
    }
}
