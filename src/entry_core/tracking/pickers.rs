//! The status, chapter and episode, and score pickers of the track sheet (T-38b-043, T-39b-040, T-39b-041, T-39b-042;
//! behaviour 8.2 to 8.4, topic track 7). The wheel and list math is `kuro_track::picker`; this file adds the rules the
//! GUI states apply on OK: the completion rule of a status write, the watching and completion rule of a progress write,
//! and the score index that rejects a string that is not on the list.

use kuro_track::picker::{NumberPicker, progress_range};

/// The chapter or episode picker: range `0..=total` when the total is known, else `0..=10000`, and it starts at the
/// current progress truncated (behaviour 8.3, `T-TRK-4`).
pub fn progress_picker(total: i64, current: f64) -> NumberPicker {
    NumberPicker::new(total, current)
}

/// The largest value the progress picker accepts (`10000` when the total is unknown).
pub fn progress_max(total: i64) -> i64 {
    *progress_range(total).end()
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
    fn a_score_string_not_on_the_list_is_refused() {
        let items: Vec<String> = ["-", "1", "2"].iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(score_index(&items, "2"), Ok(2));
        assert_eq!(score_index(&items, "7"), Err(ScoreError::NotInList), "not stored as -1");
        assert_eq!(score_index(&[], "1"), Err(ScoreError::NotInList));
        assert!(ScorePicker::new(&items, "1").is_some(), "the picker opens on a list value");
    }
}
