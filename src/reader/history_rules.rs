//! Pure rules of the reader's duplicate handling and its clock: the duplicate chapters of a completed one
//! (T-43a-031) and the wall clock the session stamps with. The read time of a session is `SessionTimer::flush`
//! (T-43a-033).
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

/// Indices of the chapters that the completed chapter's duplicates are: the other unread chapters of the list with
/// the same chapter number (E-DUP-1). The comparison is the float equality of the numbers (E-DUP-2). A completed
/// chapter with an unrecognised number (below 0) has no duplicates.
pub fn duplicate_indices(chapters: &[Rc<ReaderChapter>], completed: usize) -> Vec<usize> {
    let Some(done) = chapters.get(completed) else { return Vec::new() };
    let number = done.chapter.number;
    if number < 0.0 {
        return Vec::new();
    }
    chapters
        .iter()
        .enumerate()
        .filter(|(index, chapter)| *index != completed && !chapter.read.get() && chapter.chapter.number == number)
        .map(|(index, _)| index)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_data::item::Item;
    use kuro_platform::kind::Kind;

    fn chapter(id: i64, number: f64, read: bool) -> Rc<ReaderChapter> {
        let mut c = Item::create(Kind::Manga);
        c.id = id;
        c.url = format!("/c/{id}");
        c.number = number;
        c.done = read;
        ReaderChapter::new(c)
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
