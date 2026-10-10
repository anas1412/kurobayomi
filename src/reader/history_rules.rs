//! Pure rules of the reader's history and duplicate handling: the read time of a session (T-43a-033), the
//! duplicate chapters of a completed one (T-43a-031) and the wall clock the session stamps with.
//!
//! The session keeps the start time on the UI thread (E-HIST-2). The rules here take plain numbers, so they are
//! tested without a database or a display.

use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::model::ReaderChapter;

/// The wall clock in milliseconds since the Unix epoch. A clock before the epoch reads as 0.
pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// The read time to add for a session that started at `started_at`, at `now`. No running session adds nothing, and a
/// clock that stepped backwards adds nothing either (`max(0, now - start)`, as `HistoryRecorder::flush`).
pub fn history_duration(started_at: Option<i64>, now: i64) -> i64 {
    started_at.map_or(0, |start| (now - start).max(0))
}

/// Indices of the chapters that the completed chapter's duplicates are: the other unread chapters of the list with
/// the same chapter number (E-DUP-1). The comparison is the float equality of the numbers (E-DUP-2). A completed
/// chapter with an unrecognised number (below 0) has no duplicates.
pub fn duplicate_indices(chapters: &[Rc<ReaderChapter>], completed: usize) -> Vec<usize> {
    let Some(done) = chapters.get(completed) else { return Vec::new() };
    let number = done.chapter.chapter_number;
    if number < 0.0 {
        return Vec::new();
    }
    chapters
        .iter()
        .enumerate()
        .filter(|(index, chapter)| *index != completed && !chapter.read.get() && chapter.chapter.chapter_number == number)
        .map(|(index, _)| index)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_data::chapter::Chapter;

    fn chapter(id: i64, number: f64, read: bool) -> Rc<ReaderChapter> {
        let mut c = Chapter::create();
        c.id = id;
        c.url = format!("/c/{id}");
        c.chapter_number = number;
        c.read = read;
        ReaderChapter::new(c)
    }

    #[test]
    fn a_session_adds_the_time_since_its_start_and_nothing_without_one() {
        assert_eq!(history_duration(Some(1_000), 4_000), 3_000);
        assert_eq!(history_duration(None, 4_000), 0);
    }

    #[test]
    fn a_clock_that_stepped_back_never_subtracts_time() {
        assert_eq!(history_duration(Some(5_000), 4_000), 0);
    }

    #[test]
    fn duplicates_are_the_other_unread_chapters_with_the_same_number() {
        let list = vec![chapter(1, 5.0, false), chapter(2, 5.0, false), chapter(3, 5.0, true), chapter(4, 6.0, false)];
        assert_eq!(duplicate_indices(&list, 0), vec![1]);
    }

    #[test]
    fn the_number_comparison_is_exact_float_equality() {
        let list = vec![chapter(1, 5.0, false), chapter(2, 5.0000001, false)];
        assert!(duplicate_indices(&list, 0).is_empty());
    }

    #[test]
    fn an_unrecognised_number_has_no_duplicates() {
        let list = vec![chapter(1, -1.0, false), chapter(2, -1.0, false)];
        assert!(duplicate_indices(&list, 0).is_empty());
    }

    #[test]
    fn a_missing_completed_index_has_no_duplicates() {
        assert!(duplicate_indices(&[chapter(1, 1.0, false)], 4).is_empty());
    }
}
