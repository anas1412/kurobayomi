//! The history list view model: date headers, row text, resume and remove actions, and the texts of the empty
//! states and dialogs. Pure; the screen calls the repository and passes the rows in.

use chrono::{NaiveDate, TimeZone};

use super::model::{CoverKey, HistoryWithRelations};
use crate::dates::{ListRow, header_label, item_row_text, local_date, short_time, with_date_headers};
use crate::kind::EntryKind;

/// One history line as the screen shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryLine {
    pub history_id: i64,
    pub item_id: i64,
    pub entry_id: i64,
    pub title: String,
    /// `Ch. {n} - {time}` (`Ep.` for anime).
    pub text: String,
    pub cover: CoverKey,
}

/// The texts of one kind's history screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryTexts {
    pub empty: &'static str,
    pub remove_text: &'static str,
    pub reset_all_label: &'static str,
    pub resume_missing: &'static str,
}

pub fn texts(kind: EntryKind) -> HistoryTexts {
    match kind {
        EntryKind::Manga => HistoryTexts {
            empty: "Nothing read recently",
            remove_text: "This will remove the read date of this chapter. Are you sure?",
            reset_all_label: "Reset all chapters for this manga",
            resume_missing: "Next chapter not found",
        },
        EntryKind::Anime => HistoryTexts {
            empty: "Nothing watched recently",
            remove_text: "This will remove the watch date of this episode. Are you sure?",
            reset_all_label: "Reset all episodes for this anime",
            resume_missing: "Next Episode not found!",
        },
    }
}

/// The text of an empty list: with a search active it is "No results found", otherwise the kind's empty text.
pub fn empty_text(kind: EntryKind, search_active: bool) -> &'static str {
    if search_active { "No results found" } else { texts(kind).empty }
}

/// The date-grouped lines of the list. Headers follow the local date of `at`.
pub fn build_lines<Tz: TimeZone>(
    rows: &[HistoryWithRelations],
    kind: EntryKind,
    today: NaiveDate,
    zone: &Tz,
    relative: bool,
    date_format: Option<&str>,
) -> Vec<ListRow<HistoryLine>> {
    let prefix = match kind {
        EntryKind::Manga => "Ch.",
        EntryKind::Anime => "Ep.",
    };
    let dated: Vec<(HistoryLine, NaiveDate)> = rows
        .iter()
        .filter_map(|r| {
            let date = local_date(r.at, zone)?;
            let time = short_time(r.at, zone).unwrap_or_default();
            let line = HistoryLine {
                history_id: r.id,
                item_id: r.item_id,
                entry_id: r.entry_id,
                title: r.title.clone(),
                text: item_row_text(prefix, r.item_number, &time),
                cover: r.cover.clone(),
            };
            Some((line, date))
        })
        .collect();
    with_date_headers(dated, |date| header_label(date, today, relative, date_format))
}

/// What a row removal does: reset the one row, or every row of the entry ("Reset all" checkbox ticked).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoveAction {
    ResetOne { history_id: i64 },
    ResetEntry { entry_id: i64 },
}

pub fn remove_action(line: &HistoryLine, reset_all: bool) -> RemoveAction {
    if reset_all {
        RemoveAction::ResetEntry { entry_id: line.entry_id }
    } else {
        RemoveAction::ResetOne { history_id: line.history_id }
    }
}

/// Clear-all dialog strings and the toast after a successful clear.
pub const CLEAR_TITLE: &str = "Remove everything";
pub const CLEAR_TEXT: &str = "Are you sure? All history will be lost.";
pub const CLEAR_DONE_TOAST: &str = "History deleted";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::model::CoverKey;
    use chrono::{DateTime, Utc};

    fn row(id: i64, entry: i64, at: i64, number: f64) -> HistoryWithRelations {
        HistoryWithRelations {
            id,
            item_id: id * 10,
            entry_id: entry,
            title: format!("e{entry}"),
            item_number: number,
            at,
            duration_ms: 0,
            cover: CoverKey { entry_id: entry, source_id: 1, favorite: true, url: None, cover_last_modified: 0 },
        }
    }

    fn ms(s: &str) -> i64 {
        DateTime::parse_from_rfc3339(s).unwrap().timestamp_millis()
    }

    #[test]
    fn headers_appear_at_each_local_date_change_with_row_text() {
        let rows = vec![
            row(1, 1, ms("2025-03-02T23:59:00Z"), 12.0),
            row(2, 1, ms("2025-03-02T00:01:00Z"), 11.5),
            row(3, 2, ms("2025-03-01T12:00:00Z"), 0.5),
        ];
        let today = NaiveDate::from_ymd_opt(2025, 3, 10).unwrap();
        let lines = build_lines(&rows, EntryKind::Manga, today, &Utc, false, None);
        let headers: Vec<&str> = lines
            .iter()
            .filter_map(|l| match l {
                ListRow::Header(h) => Some(h.as_str()),
                ListRow::Item(_) => None,
            })
            .collect();
        assert_eq!(headers, vec!["2025-03-02", "2025-03-01"]);
        let first = match &lines[1] {
            ListRow::Item(line) => line.text.clone(),
            ListRow::Header(_) => panic!("expected an item"),
        };
        assert_eq!(first, "Ch. 12 - 23:59");
    }

    #[test]
    fn the_empty_text_depends_on_search_and_kind() {
        assert_eq!(empty_text(EntryKind::Manga, false), "Nothing read recently");
        assert_eq!(empty_text(EntryKind::Anime, false), "Nothing watched recently");
        assert_eq!(empty_text(EntryKind::Anime, true), "No results found");
        assert_eq!(texts(EntryKind::Anime).resume_missing, "Next Episode not found!");
    }

    #[test]
    fn remove_actions_pick_the_entry_or_the_row() {
        let line = HistoryLine {
            history_id: 4,
            item_id: 40,
            entry_id: 9,
            title: String::new(),
            text: String::new(),
            cover: CoverKey::create(),
        };
        assert_eq!(remove_action(&line, false), RemoveAction::ResetOne { history_id: 4 });
        assert_eq!(remove_action(&line, true), RemoveAction::ResetEntry { entry_id: 9 });
        assert_eq!(CLEAR_TEXT, "Are you sure? All history will be lost.");
    }
}
