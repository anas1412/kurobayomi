//! Bulk and per-row download actions of the Updates feed (T-42a-031). Pure: this file turns the selected rows into the
//! work to do; the download queue and the disk are the caller's. The grouped work runs detached, so once a plan is
//! handed over it is not cancelled by the screen.

use crate::updates::download_state::DownloadState;

/// The actions of a download indicator or of the bulk bar (`ChapterDownloadAction` / `EpisodeDownloadAction`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadAction {
    Start,
    StartNow,
    Cancel,
    Delete,
    /// Opens the episode options sheet of one anime unit. It is a dialog, not a queue step, so it has no [`Plan`].
    ShowQualities,
}

/// One selected unit: its entry, its id and its current download state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitTarget {
    pub entry_id: i64,
    pub unit_id: i64,
    pub state: DownloadState,
}

/// What the caller knows about an entry: whether it is in the library, and whether its source is installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryFacts {
    pub in_library: bool,
    pub source_installed: bool,
}

/// One step for the caller to run, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Enqueue these units of one entry (in selection order).
    Enqueue { entry_id: i64, unit_ids: Vec<i64> },
    /// Start the queue (a START over an ERROR row restarts it).
    StartQueue,
    /// Start this one unit now.
    StartNow { unit_id: i64 },
    /// Remove this unit from the queue (no in-place status change, EC-42-U-07).
    CancelQueued { unit_id: i64 },
    /// Delete the downloads of these units of one entry, with the installed source.
    Delete { entry_id: i64, unit_ids: Vec<i64> },
}

/// The units of each entry, in order of first appearance.
fn by_entry(rows: &[UnitTarget]) -> Vec<(i64, Vec<i64>)> {
    let mut groups: Vec<(i64, Vec<i64>)> = Vec::new();
    for row in rows {
        match groups.iter_mut().find(|(entry, _)| *entry == row.entry_id) {
            Some((_, units)) => units.push(row.unit_id),
            None => groups.push((row.entry_id, vec![row.unit_id])),
        }
    }
    groups
}

/// Whether a unit has a queue entry (the states that come from the queue).
fn is_queued(state: DownloadState) -> bool {
    matches!(state, DownloadState::Queue | DownloadState::Downloading | DownloadState::Error)
}

/// The plan of one action over the selected rows.
///
/// - `Start` groups by entry and skips entries that are not in the library or whose source is not installed. It also
///   starts the queue when a row is in ERROR.
/// - `StartNow` and `Cancel` need exactly one row; `Cancel` also needs that row to be queued.
/// - `Delete` groups by entry and skips entries whose source is not installed (no message, EC-42-T-15 for storage).
///   The library check does not apply to delete.
pub fn plan(action: DownloadAction, rows: &[UnitTarget], facts: impl Fn(i64) -> Option<EntryFacts>) -> Vec<Plan> {
    if rows.is_empty() {
        return Vec::new();
    }
    match action {
        DownloadAction::Start => {
            let mut steps: Vec<Plan> = by_entry(rows)
                .into_iter()
                .filter(|(entry, _)| facts(*entry).is_some_and(|f| f.in_library && f.source_installed))
                .map(|(entry_id, unit_ids)| Plan::Enqueue { entry_id, unit_ids })
                .collect();
            if rows.iter().any(|r| r.state == DownloadState::Error) {
                steps.push(Plan::StartQueue);
            }
            steps
        }
        DownloadAction::StartNow => match rows {
            [only] => vec![Plan::StartNow { unit_id: only.unit_id }],
            _ => Vec::new(),
        },
        DownloadAction::Cancel => match rows {
            [only] if is_queued(only.state) => vec![Plan::CancelQueued { unit_id: only.unit_id }],
            _ => Vec::new(),
        },
        DownloadAction::Delete => by_entry(rows)
            .into_iter()
            .filter(|(entry, _)| facts(*entry).is_some_and(|f| f.source_installed))
            .map(|(entry_id, unit_ids)| Plan::Delete { entry_id, unit_ids })
            .collect(),
        DownloadAction::ShowQualities => Vec::new(),
    }
}

/// The entries a Delete over `rows` skips because their source is not installed, in order of first appearance
/// (T-42b-010): the caller logs a warning and shows the toast once for them.
pub fn delete_skipped(rows: &[UnitTarget], facts: impl Fn(i64) -> Option<EntryFacts>) -> Vec<i64> {
    by_entry(rows).into_iter().map(|(entry, _)| entry).filter(|entry| !facts(*entry).is_some_and(|f| f.source_installed)).collect()
}

/// The downloads a mark-as-read removes when the option "remove after marked as read" is on (E-U33): only the units
/// that were newly read (the caller passes the rows the read write changes) and that are downloaded, grouped per entry.
pub fn remove_after_read(newly_read: &[UnitTarget]) -> Vec<(i64, Vec<i64>)> {
    let downloaded: Vec<UnitTarget> = newly_read.iter().copied().filter(|r| r.state == DownloadState::Downloaded).collect();
    by_entry(&downloaded)
}

/// SHOW_QUALITIES (T-42b-009, T-42b-019): the episode options sheet opens only for exactly one row. Returns that row;
/// a multi-select gives nothing and leaves the selection as it is (E-U49).
pub fn quality_target(rows: &[UnitTarget]) -> Option<UnitTarget> {
    match rows {
        [only] => Some(*only),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(entry_id: i64, unit_id: i64, state: DownloadState) -> UnitTarget {
        UnitTarget { entry_id, unit_id, state }
    }

    fn all_good(_: i64) -> Option<EntryFacts> {
        Some(EntryFacts { in_library: true, source_installed: true })
    }

    #[test]
    fn download_start_groups_by_entry_and_skips_bad_entries() {
        let rows = vec![
            unit(1, 10, DownloadState::NotDownloaded),
            unit(2, 20, DownloadState::NotDownloaded),
            unit(1, 11, DownloadState::NotDownloaded),
            unit(3, 30, DownloadState::NotDownloaded),
        ];
        let facts = |entry: i64| match entry {
            1 => Some(EntryFacts { in_library: true, source_installed: true }),
            2 => Some(EntryFacts { in_library: false, source_installed: true }),
            _ => Some(EntryFacts { in_library: true, source_installed: false }),
        };
        assert_eq!(plan(DownloadAction::Start, &rows, facts), vec![Plan::Enqueue { entry_id: 1, unit_ids: vec![10, 11] }]);
    }

    #[test]
    fn download_start_over_an_error_row_also_starts_the_queue() {
        let rows = vec![unit(1, 10, DownloadState::Error), unit(1, 11, DownloadState::NotDownloaded)];
        assert_eq!(
            plan(DownloadAction::Start, &rows, all_good),
            vec![Plan::Enqueue { entry_id: 1, unit_ids: vec![10, 11] }, Plan::StartQueue]
        );
    }

    #[test]
    fn download_start_skips_an_entry_with_no_facts() {
        let rows = vec![unit(5, 50, DownloadState::NotDownloaded)];
        assert!(plan(DownloadAction::Start, &rows, |_| None).is_empty());
    }

    #[test]
    fn download_start_now_and_cancel_need_exactly_one_row() {
        let two = vec![unit(1, 10, DownloadState::Queue), unit(1, 11, DownloadState::Queue)];
        assert!(plan(DownloadAction::StartNow, &two, all_good).is_empty());
        assert!(plan(DownloadAction::Cancel, &two, all_good).is_empty());
        let one = vec![unit(1, 10, DownloadState::NotDownloaded)];
        assert_eq!(plan(DownloadAction::StartNow, &one, all_good), vec![Plan::StartNow { unit_id: 10 }]);
        assert!(plan(DownloadAction::Cancel, &one, all_good).is_empty(), "nothing is queued for this row");
    }

    #[test]
    fn download_cancel_removes_the_queue_entry() {
        let queued = vec![unit(1, 10, DownloadState::Downloading)];
        assert_eq!(plan(DownloadAction::Cancel, &queued, all_good), vec![Plan::CancelQueued { unit_id: 10 }]);
    }

    #[test]
    fn download_delete_groups_by_entry_with_the_installed_source() {
        let rows = vec![unit(1, 10, DownloadState::Downloaded), unit(2, 20, DownloadState::Downloaded), unit(1, 11, DownloadState::Downloaded)];
        let facts = |entry: i64| Some(EntryFacts { in_library: false, source_installed: entry == 1 });
        assert_eq!(plan(DownloadAction::Delete, &rows, facts), vec![Plan::Delete { entry_id: 1, unit_ids: vec![10, 11] }]);
    }

    #[test]
    fn download_remove_after_read_only_newly_read_downloads() {
        // Only the rows the read write changes are passed in (already-read rows are filtered out by the caller).
        let newly_read = vec![unit(1, 10, DownloadState::Downloaded), unit(1, 11, DownloadState::NotDownloaded), unit(2, 20, DownloadState::Downloaded)];
        assert_eq!(remove_after_read(&newly_read), vec![(1, vec![10]), (2, vec![20])]);
    }

    #[test]
    fn download_delete_names_the_entries_it_skips() {
        let rows = [unit(1, 10, DownloadState::Downloaded), unit(2, 20, DownloadState::Downloaded), unit(2, 21, DownloadState::Downloaded), unit(3, 30, DownloadState::Downloaded)];
        let facts = |entry: i64| match entry {
            1 => Some(EntryFacts { in_library: true, source_installed: true }),
            2 => Some(EntryFacts { in_library: true, source_installed: false }),
            _ => None,
        };
        assert_eq!(delete_skipped(&rows, facts), vec![2, 3]);
        assert!(delete_skipped(&rows, all_good).is_empty());
    }

    #[test]
    fn show_qualities_is_a_dialog_not_a_plan() {
        assert!(plan(DownloadAction::ShowQualities, &[unit(1, 10, DownloadState::NotDownloaded)], all_good).is_empty());
    }

    #[test]
    fn quality_target_needs_exactly_one_row() {
        let one = unit(1, 10, DownloadState::NotDownloaded);
        assert_eq!(quality_target(&[one]), Some(one));
        assert_eq!(quality_target(&[one, unit(2, 11, DownloadState::Queue)]), None);
        assert_eq!(quality_target(&[]), None);
    }
}
