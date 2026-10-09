//! The rows of the Updates feed and how a gesture changes the selection (T-42a-030, T-42a-034). Pure. A feed row
//! carries its own selected flag, as [`Selection`] expects; a press is routed here by what it was (tap, long press,
//! Ctrl or Shift click) and whether the selection mode is already on.

use chrono::{NaiveDate, TimeZone};
use kuro_domain::dates::ListRow;
use kuro_domain::updates::model::UpdateRow;

use crate::updates::bulk::UnitFlags;
use crate::updates::dates::grouped;
use crate::updates::selection::{Selectable, Selection, is_selection_mode};

#[derive(Clone, Debug, PartialEq)]
pub struct FeedRow {
    pub row: UpdateRow,
    pub selected: bool,
}

impl Selectable for FeedRow {
    fn id(&self) -> i64 {
        self.row.item_id
    }
    fn selected(&self) -> bool {
        self.selected
    }
    fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }
}

/// A new list from the query. Rows that were selected stay selected by unit id (EC-42-U-05), and the bounds are
/// recomputed from the flags.
pub fn replace_rows(rows: Vec<UpdateRow>, previous: &[FeedRow], selection: &mut Selection) -> Vec<FeedRow> {
    let was_selected = |id: i64| previous.iter().any(|p| p.selected && p.row.item_id == id);
    let next: Vec<FeedRow> = rows.into_iter().map(|row| FeedRow { selected: was_selected(row.item_id), row }).collect();
    selection.sync(&next);
    next
}

/// The list under date headers by fetch day; each item is an index into `rows`.
pub fn layout<Tz: TimeZone>(
    rows: &[FeedRow],
    zone: &Tz,
    today: NaiveDate,
    relative: bool,
    date_pref: &str,
    locale: &str,
) -> Vec<ListRow<usize>> {
    grouped(rows.iter().enumerate().map(|(i, r)| (i, r.row.date_fetch)), zone, today, relative, date_pref, locale)
}

/// The flags of the selected rows, for the bulk planner.
pub fn selected_flags(rows: &[FeedRow]) -> Vec<UnitFlags> {
    rows.iter()
        .filter(|r| r.selected)
        .map(|r| UnitFlags {
            id: r.row.item_id,
            read: r.row.done,
            bookmark: r.row.bookmark,
            fillermark: r.row.fillermark,
            progress: r.row.progress,
        })
        .collect()
}

/// How a row was pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Tap,
    LongPress,
    CtrlClick,
    ShiftClick,
}

/// What a press did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// Not in selection mode: the tap opens the unit.
    Open,
    /// The selection changed.
    Changed,
    /// Nothing to do (an unknown row).
    Nothing,
}

/// Routes a press on the row with unit `id`.
/// - Tap opens the unit, or toggles the row while the selection mode is on.
/// - Long press toggles the row and fills the gap to the existing selection (`fromLongPress`).
/// - Ctrl+click toggles the row, in or out of the selection mode.
/// - Shift+click selects the range from the existing selection to the row.
pub fn press(gesture: Gesture, selection: &mut Selection, rows: &mut [FeedRow], id: i64) -> Press {
    let Some(selected) = rows.iter().find(|r| r.row.item_id == id).map(|r| r.selected) else {
        return Press::Nothing;
    };
    let mode = is_selection_mode(rows);
    let changed = match gesture {
        Gesture::Tap if !mode => return Press::Open,
        Gesture::Tap | Gesture::CtrlClick => selection.toggle(rows, id, !selected, true, false),
        Gesture::LongPress => selection.toggle(rows, id, !selected, true, true),
        Gesture::ShiftClick => selection.toggle(rows, id, true, true, true),
    };
    if !mode {
        // a first selection that was not a long press leaves a half-set bound; read both from the flags
        selection.sync(rows);
    }
    if changed.is_empty() { Press::Nothing } else { Press::Changed }
}

#[cfg(test)]
mod tests {
    use kuro_domain::kind::EntryKind;

    use super::*;

    fn update(id: i64) -> UpdateRow {
        UpdateRow {
            kind: EntryKind::Manga,
            entry_id: 1,
            entry_title: "T".into(),
            item_id: id,
            item_name: format!("c{id}"),
            scanlator: None,
            done: false,
            bookmark: false,
            fillermark: false,
            progress: 0,
            total_seconds: 0,
            source_id: 1,
            date_fetch: 0,
            date_upload: 0,
            thumbnail_url: None,
            cover_last_modified: 0,
        }
    }

    fn rows(n: i64) -> Vec<FeedRow> {
        (0..n).map(|id| FeedRow { row: update(id), selected: false }).collect()
    }

    fn flags(rows: &[FeedRow]) -> Vec<bool> {
        rows.iter().map(|r| r.selected).collect()
    }

    #[test]
    fn feed_tap_opens_until_the_selection_mode_is_on() {
        let (mut items, mut sel) = (rows(4), Selection::default());
        assert_eq!(press(Gesture::Tap, &mut sel, &mut items, 1), Press::Open);
        assert_eq!(press(Gesture::LongPress, &mut sel, &mut items, 1), Press::Changed);
        assert_eq!(press(Gesture::Tap, &mut sel, &mut items, 3), Press::Changed, "tap toggles while selecting");
        assert_eq!(flags(&items), [false, true, false, true]);
        assert_eq!(press(Gesture::Tap, &mut sel, &mut items, 3), Press::Changed);
        assert_eq!(flags(&items), [false, true, false, false]);
    }

    #[test]
    fn feed_long_press_fills_the_range() {
        let (mut items, mut sel) = (rows(6), Selection::default());
        press(Gesture::LongPress, &mut sel, &mut items, 1);
        press(Gesture::LongPress, &mut sel, &mut items, 4);
        assert_eq!(flags(&items), [false, true, true, true, true, false]);
    }

    #[test]
    fn feed_ctrl_click_toggles_without_the_selection_mode() {
        let (mut items, mut sel) = (rows(3), Selection::default());
        assert_eq!(press(Gesture::CtrlClick, &mut sel, &mut items, 2), Press::Changed);
        assert_eq!(flags(&items), [false, false, true]);
        assert_eq!(sel.bounds(), [2, 2]);
        press(Gesture::CtrlClick, &mut sel, &mut items, 0);
        assert_eq!(flags(&items), [true, false, true], "ctrl adds a single row, no range");
        press(Gesture::CtrlClick, &mut sel, &mut items, 2);
        assert_eq!(flags(&items), [true, false, false]);
    }

    #[test]
    fn feed_shift_click_selects_a_range_and_never_deselects() {
        let (mut items, mut sel) = (rows(6), Selection::default());
        press(Gesture::CtrlClick, &mut sel, &mut items, 1);
        press(Gesture::ShiftClick, &mut sel, &mut items, 4);
        assert_eq!(flags(&items), [false, true, true, true, true, false]);
        assert_eq!(press(Gesture::ShiftClick, &mut sel, &mut items, 4), Press::Nothing, "already selected");
    }

    #[test]
    fn feed_unknown_row_is_ignored() {
        let (mut items, mut sel) = (rows(2), Selection::default());
        assert_eq!(press(Gesture::Tap, &mut sel, &mut items, 99), Press::Nothing);
    }

    #[test]
    fn feed_new_list_keeps_selected_ids_and_recomputes_bounds() {
        let (mut items, mut sel) = (rows(5), Selection::default());
        press(Gesture::LongPress, &mut sel, &mut items, 2);
        press(Gesture::LongPress, &mut sel, &mut items, 3);
        let fresh = replace_rows(vec![update(9), update(3), update(2), update(8)], &items, &mut sel);
        assert_eq!(flags(&fresh), [false, true, true, false]);
        assert_eq!(sel.bounds(), [1, 2]);
    }

    #[test]
    fn feed_selected_flags_feed_the_bulk_planner() {
        let mut items = rows(3);
        items[1].selected = true;
        items[1].row.bookmark = true;
        items[1].row.progress = 7;
        let picked = selected_flags(&items);
        assert_eq!(picked.len(), 1);
        assert_eq!((picked[0].id, picked[0].bookmark, picked[0].progress), (1, true, 7));
    }
}
