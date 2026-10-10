//! The status, chapter and episode, and score pickers of the track sheet (T-38b-043, T-39b-040, T-39b-041, T-39b-042;
//! behaviour 8.2 to 8.4, topic track 7). The wheel and list math is `kuro_track::picker`; this file adds the rules the
//! GUI states apply on OK: the completion rule of a status write, the watching and completion rule of a progress write,
//! and the score index that rejects a string that is not on the list.

use kuro_track::picker::{NumberPicker, StatusPicker, StatusRow, progress_range};

/// The status picker opened on `current`, in the tracker's order (behaviour 8.2).
pub fn status_picker(rows: Vec<StatusRow>, current: i64) -> StatusPicker {
    StatusPicker::new(rows, current)
}

/// The chapter or episode picker: range `0..=total` when the total is known, else `0..=10000`, and it starts at the
/// current progress truncated (behaviour 8.3, `T-TRK-4`).
pub fn progress_picker(total: i64, current: f64) -> NumberPicker {
    NumberPicker::new(total, current)
}

/// The largest value the progress picker accepts (`10000` when the total is unknown).
pub fn progress_max(total: i64) -> i64 {
    *progress_range(total).end()
}

/// What a status write stores beside the status (behaviour 3.2): a completed status with a known total sets the
/// progress to the total.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusWrite {
    pub status: i64,
    /// The progress to store, when the write sets it.
    pub progress: Option<i64>,
}

pub fn status_write(status: i64, completion: i64, total: i64) -> StatusWrite {
    let progress = (status == completion && total != 0).then_some(total);
    StatusWrite { status, progress }
}

/// What a progress write stores beside the progress (behaviour 3.3, `T-TRK-1`, E-TRK-6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgressWrite {
    pub progress: i64,
    pub status: i64,
    /// The finish date to stamp (`now_ms`) when the progress reaches the total.
    pub finish_ms: Option<i64>,
}

/// Plain form of `kuro_track::remote::set_last_episode_seen`, over ids so it can be tested without a tracker. An
/// unstarted entry moves to watching unless it is being rewatched; reaching the total completes it and stamps the
/// finish date.
#[allow(clippy::too_many_arguments)]
pub fn progress_write(
    current_progress: f64,
    current_status: i64,
    progress: i64,
    total: i64,
    watching: i64,
    rewatching: i64,
    completion: i64,
    now_ms: i64,
) -> ProgressWrite {
    let mut status = current_status;
    if current_progress == 0.0 && current_progress < progress as f64 && current_status != rewatching {
        status = watching;
    }
    if total != 0 && progress == total {
        return ProgressWrite { progress, status: completion, finish_ms: Some(now_ms) };
    }
    ProgressWrite { progress, status, finish_ms: None }
}

/// The index of `text` in the tracker's score list. A string that is not on the list is refused, not stored as -1
/// (E-TRK-7, D-4). An empty list has no picker, so every text is refused.
pub fn score_index(items: &[String], text: &str) -> Result<usize, ScoreError> {
    items.iter().position(|s| s == text).ok_or(ScoreError::NotInList)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoreError {
    NotInList,
}

#[cfg(test)]
mod tests {
    use kuro_track::picker::{NumberKey, ScorePicker};

    use super::*;

    fn row(status: i64) -> StatusRow {
        StatusRow { status, label_key: None }
    }

    #[test]
    fn status_options_keep_the_tracker_order_and_start_at_the_current_status() {
        let mut p = status_picker(vec![row(1), row(2), row(3)], 2);
        assert_eq!(p.selection(), 2);
        assert_eq!(p.rows().iter().map(|r| r.status).collect::<Vec<_>>(), vec![1, 2, 3]);
        p.select(3);
        assert_eq!(p.confirm(), 3, "OK returns the chosen status, cancel is simply not calling confirm");
    }

    #[test]
    fn completing_a_status_with_a_known_total_sets_the_progress_to_the_total() {
        assert_eq!(status_write(2, 2, 24), StatusWrite { status: 2, progress: Some(24) });
        assert_eq!(status_write(2, 2, 0), StatusWrite { status: 2, progress: None }, "an unknown total sets nothing");
        assert_eq!(status_write(1, 2, 24), StatusWrite { status: 1, progress: None });
    }

    #[test]
    fn chapter_picker_max_default_10000() {
        assert_eq!(progress_max(0), 10_000, "an unknown total gives 10000");
        assert_eq!(progress_max(-1), 10_000);
        assert_eq!(progress_max(12), 12);
        assert_eq!(progress_picker(0, 5.0).max(), 10_000);
    }

    #[test]
    fn chapter_picker_initial_truncates() {
        let p = progress_picker(40, 12.9);
        assert_eq!(p.value(), 12, "the fraction is truncated");
        assert_eq!(p.committed(), None, "an unchanged number commits nothing, so 12.9 stays");
        let mut q = progress_picker(40, 12.9);
        q.press(NumberKey::Home);
        assert_eq!(q.committed(), Some(0));
    }

    #[test]
    fn progress_write_moves_an_unstarted_entry_to_watching() {
        let w = progress_write(0.0, 99, 3, 0, 1, 2, 9, 100);
        assert_eq!(w, ProgressWrite { progress: 3, status: 1, finish_ms: None });
    }

    #[test]
    fn progress_write_completes_at_the_total_and_stamps_the_finish() {
        let w = progress_write(11.0, 1, 12, 12, 1, 2, 9, 777);
        assert_eq!(w, ProgressWrite { progress: 12, status: 9, finish_ms: Some(777) });
        let rewatch = progress_write(0.0, 2, 3, 0, 1, 2, 9, 777);
        assert_eq!(rewatch.status, 2, "a rewatch keeps its status");
    }

    #[test]
    fn a_score_string_not_on_the_list_is_refused() {
        let items: Vec<String> = ["-", "1", "2"].iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(score_index(&items, "2"), Ok(2));
        assert_eq!(score_index(&items, "7"), Err(ScoreError::NotInList), "not stored as -1");
        assert_eq!(score_index(&[], "1"), Err(ScoreError::NotInList));
        assert!(ScorePicker::new(&items, "1").is_some(), "the picker opens on a list value");
    }
}
