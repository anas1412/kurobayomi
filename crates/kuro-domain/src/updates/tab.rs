//! The Updates tab state: date-grouped rows, the selection span, the bookmark targets and the messages of the
//! "Update library" and "Update category" actions. Pure; the screen supplies the rows and the clock.

use std::collections::BTreeSet;

use chrono::{NaiveDate, TimeZone};

use super::model::UpdateRow;
use crate::dates::{ListRow, header_label, local_date, with_date_headers};

/// One update line: the entry and item it names, and the state the row shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateLine {
    pub entry_id: i64,
    pub item_id: i64,
    pub entry_title: String,
    pub text: String,
    pub done: bool,
    pub bookmark: bool,
    pub fillermark: bool,
    pub source_id: i64,
}

/// Groups the rows by the local date of `date_fetch`. The row text is the item name (the feed carries no number).
pub fn group<Tz: TimeZone>(
    rows: &[UpdateRow],
    today: NaiveDate,
    zone: &Tz,
    relative: bool,
    date_format: Option<&str>,
) -> Vec<ListRow<UpdateLine>> {
    let dated: Vec<(UpdateLine, NaiveDate)> = rows
        .iter()
        .filter_map(|r| {
            let date = local_date(r.date_fetch, zone)?;
            let line = UpdateLine {
                entry_id: r.entry_id,
                item_id: r.item_id,
                entry_title: r.entry_title.clone(),
                text: r.item_name.clone(),
                done: r.done,
                bookmark: r.bookmark,
                fillermark: r.fillermark,
                source_id: r.source_id,
            };
            Some((line, date))
        })
        .collect();
    with_date_headers(dated, |date| header_label(date, today, relative, date_format))
}

/// The selection span of a long press (`toggleSelection` with `fromLongPress`): the ids between the anchor and the
/// pressed row, inclusive, that are not selected yet. Without an anchor only the pressed row is added.
pub fn span_to_add(order: &[i64], selected: &BTreeSet<i64>, anchor: Option<usize>, pressed: usize) -> Vec<i64> {
    let Some(from) = anchor else {
        return order.get(pressed).copied().filter(|id| !selected.contains(id)).into_iter().collect();
    };
    let (lo, hi) = if from <= pressed { (from, pressed) } else { (pressed, from) };
    order
        .iter()
        .enumerate()
        .filter(|(i, _)| *i >= lo && *i <= hi)
        .map(|(_, id)| *id)
        .filter(|id| !selected.contains(id))
        .collect()
}

/// Bookmark or unbookmark a selection: only the items whose flag differs from `target` change.
pub fn bookmark_targets(items: &[(i64, bool)], target: bool) -> Vec<i64> {
    items.iter().filter(|(_, flag)| *flag != target).map(|(id, _)| *id).collect()
}

/// The toolbar message of "Update library" or "Update category".
pub fn update_started_message(scope: &str, started: bool) -> String {
    if started { format!("Updating {scope}") } else { "An update is already running".to_owned() }
}

/// The count on the navigation item: manga plus anime when the count is switched on, hidden at 0.
pub fn nav_badge(show_count: bool, manga_new: i32, anime_new: i32) -> Option<i32> {
    let total = manga_new + anime_new;
    (show_count && total > 0).then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use kuro_platform::kind::Kind;

    #[test]
    fn a_long_press_selects_the_span_between_the_anchor_and_the_pressed_row() {
        let order = [10, 11, 12, 13, 14];
        let selected: BTreeSet<i64> = [12].into_iter().collect();
        assert_eq!(span_to_add(&order, &selected, Some(1), 4), vec![11, 13, 14]);
        assert_eq!(span_to_add(&order, &selected, Some(4), 1), vec![11, 13, 14]);
        assert_eq!(span_to_add(&order, &selected, None, 0), vec![10]);
    }

    #[test]
    fn bookmark_touches_only_items_whose_flag_differs() {
        let items = [(1, false), (2, true), (3, false)];
        assert_eq!(bookmark_targets(&items, true), vec![1, 3]);
        assert_eq!(bookmark_targets(&items, false), vec![2]);
    }

    #[test]
    fn messages_and_badge_follow_the_counters() {
        assert_eq!(update_started_message("library", true), "Updating library");
        assert_eq!(update_started_message("category", false), "An update is already running");
        assert_eq!(nav_badge(true, 2, 3), Some(5));
        assert_eq!(nav_badge(true, 0, 0), None);
        assert_eq!(nav_badge(false, 2, 3), None);
    }

    fn feed_row(item_id: i64, date_fetch: i64, name: &str) -> UpdateRow {
        UpdateRow {
            kind: Kind::Manga,
            entry_id: 1,
            entry_title: "Show".into(),
            item_id,
            item_name: name.into(),
            scanlator: None,
            done: false,
            bookmark: false,
            fillermark: false,
            progress: 0,
            total_seconds: 0,
            source_id: 7,
            date_fetch,
            date_upload: 0,
            thumbnail_url: None,
            cover_last_modified: 0,
        }
    }

    #[test]
    fn rows_group_by_the_local_fetch_date_under_the_shared_header_rule() {
        let day = 86_400_000i64;
        let rows = vec![feed_row(1, 10 * day, "a"), feed_row(2, 10 * day + 1000, "b"), feed_row(3, 9 * day, "c")];
        let today = NaiveDate::from_ymd_opt(1970, 1, 11).unwrap();
        let grouped = group(&rows, today, &Utc, true, None);
        let kinds: Vec<&str> = grouped
            .iter()
            .map(|r| match r {
                ListRow::Header(h) => h.as_str(),
                ListRow::Item(l) => l.text.as_str(),
            })
            .collect();
        assert_eq!(kinds, vec!["Today", "a", "b", "Yesterday", "c"]);
    }
}
