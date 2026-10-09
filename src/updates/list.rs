//! List primitives (T-42a-005, T-42a-008): the date-grouped row model, the page state that picks the child of the
//! page `gtk::Stack`, and the keys of rows. Pure; the widgets read these and never build the rules themselves.

use chrono::NaiveDate;
use kuro_domain::dates::ListRow;

/// Groups items by the local date of their timestamp (`insertSeparators`). A header goes before the first dated item
/// and before every dated item whose date differs from the previous dated item. An item with no date never creates a
/// header and never changes the current date, but stays in the list.
pub fn group_by_date<T>(items: Vec<(T, Option<NaiveDate>)>, label: impl Fn(NaiveDate) -> String) -> Vec<ListRow<T>> {
    let mut out = Vec::with_capacity(items.len() * 2);
    let mut previous: Option<NaiveDate> = None;
    for (item, date) in items {
        if let Some(date) = date
            && previous != Some(date)
        {
            out.push(ListRow::Header(label(date)));
            previous = Some(date);
        }
        out.push(ListRow::Item(item));
    }
    out
}

/// Positions of the item rows. Headers are not selectable and are not counted by range selection, so a range is
/// taken over these positions only.
pub fn selectable_positions<T>(rows: &[ListRow<T>]) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| matches!(row, ListRow::Item(_)))
        .map(|(position, _)| position)
        .collect()
}

/// The stable key of a row: `(entry_id, unit_id)` for an item, the header text for a date header.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RowKey {
    Header(String),
    Item { entry_id: i64, unit_id: i64 },
}

pub fn item_key(entry_id: i64, unit_id: i64) -> RowKey {
    RowKey::Item { entry_id, unit_id }
}

/// What the page shows: the loading screen until the first load, the empty message when the list has no rows, the
/// error row after a failed load, or the list. `child_name` is the name of the matching child of the `gtk::Stack`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageState {
    Loading,
    Empty(String),
    /// A failed load: the message, and a retry the page offers.
    Error(String),
    List,
}

impl PageState {
    pub fn for_load(loaded: bool, row_count: usize, empty_message: &str) -> Self {
        if !loaded {
            PageState::Loading
        } else if row_count == 0 {
            PageState::Empty(empty_message.to_owned())
        } else {
            PageState::List
        }
    }

    pub fn child_name(&self) -> &'static str {
        match self {
            PageState::Loading => "loading",
            PageState::Empty(_) => "empty",
            PageState::Error(_) => "error",
            PageState::List => "list",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    fn label(date: NaiveDate) -> String {
        format!("day {}", date.format("%d"))
    }

    #[test]
    fn group_by_local_date_inserts_leading_header() {
        let rows = group_by_date(vec![("a", Some(d(9))), ("b", Some(d(9))), ("c", Some(d(8)))], label);
        assert_eq!(
            rows,
            vec![
                ListRow::Header("day 09".into()),
                ListRow::Item("a"),
                ListRow::Item("b"),
                ListRow::Header("day 08".into()),
                ListRow::Item("c"),
            ]
        );
    }

    #[test]
    fn group_by_local_date_no_header_for_null_date() {
        let rows = group_by_date(vec![("x", None), ("a", Some(d(9))), ("y", None), ("b", Some(d(9)))], label);
        assert_eq!(
            rows,
            vec![
                ListRow::Item("x"),
                ListRow::Header("day 09".into()),
                ListRow::Item("a"),
                ListRow::Item("y"),
                ListRow::Item("b"),
            ]
        );
    }

    #[test]
    fn selectable_positions_skip_headers() {
        let rows = group_by_date(vec![("a", Some(d(9))), ("b", Some(d(8)))], label);
        assert_eq!(selectable_positions(&rows), vec![1, 3]);
    }

    #[test]
    fn row_keys_use_entry_and_unit_ids() {
        assert_eq!(item_key(7, 9), RowKey::Item { entry_id: 7, unit_id: 9 });
        assert_ne!(item_key(7, 9), item_key(9, 7));
    }

    #[test]
    fn page_state_picks_loading_empty_or_list() {
        assert_eq!(PageState::for_load(false, 0, "none").child_name(), "loading");
        assert_eq!(PageState::for_load(true, 0, "No recent updates"), PageState::Empty("No recent updates".to_owned()));
        assert_eq!(PageState::for_load(true, 0, "none").child_name(), "empty");
        assert_eq!(PageState::for_load(true, 3, "none").child_name(), "list");
    }
}
