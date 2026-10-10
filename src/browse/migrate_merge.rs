//! The chapter-state copy of a migration (behavior 15 step 2; T-40a-016): `merge_chapter_state` carries the fetch date and
//! the bookmark of the old chapter over to the new chapter with the same number, and marks the new chapters read up to the
//! highest read number of the old ones. Pure: the engine reads the rows and writes the result (`p1-rust-design.md`).

use kuro_data::chapter::Chapter;
use kuro_domain::items::model::ChapterExt;

/// The new chapters with the old state merged in. The read flag is never copied: it is derived from the old read point
/// (`<=` the highest read number). A chapter whose number is not recognised (below zero) is left as it is, and the first
/// old chapter with the same number supplies the date and the bookmark.
pub fn merge_chapter_state(prev: &[Chapter], new: &[Chapter]) -> Vec<Chapter> {
    let max_read = prev.iter().filter(|c| c.read).map(|c| c.chapter_number).reduce(f64::max);
    new.iter()
        .map(|c| {
            let mut merged = c.clone();
            if c.is_recognized_number() {
                if let Some(old) = prev.iter().find(|p| p.is_recognized_number() && p.chapter_number == c.chapter_number) {
                    merged.date_fetch = old.date_fetch;
                    merged.bookmark = old.bookmark;
                }
                if max_read.is_some_and(|max| merged.chapter_number <= max) {
                    merged.read = true;
                }
            }
            merged
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(number: f64, read: bool) -> Chapter {
        Chapter { chapter_number: number, read, ..Chapter::create() }
    }

    #[test]
    fn a_new_chapter_takes_the_fetch_date_and_bookmark_of_the_old_one() {
        let old = [Chapter { date_fetch: 500, bookmark: true, ..chapter(3.0, false) }];
        let merged = merge_chapter_state(&old, &[chapter(3.0, false)]);
        assert_eq!((merged[0].date_fetch, merged[0].bookmark), (500, true), "TC-40-020");
    }

    #[test]
    fn chapters_at_or_below_the_highest_read_number_are_marked_read() {
        let old = [chapter(3.0, true), chapter(5.0, false)];
        let merged = merge_chapter_state(&old, &[chapter(2.0, false), chapter(3.0, false), chapter(4.0, false)]);
        assert_eq!(merged.iter().map(|c| c.read).collect::<Vec<_>>(), [true, true, false], "TC-40-021, E-40-80: equal is read");
    }

    #[test]
    fn the_first_old_chapter_with_the_same_number_wins() {
        let old = [Chapter { date_fetch: 100, bookmark: true, ..chapter(5.0, false) }, Chapter { date_fetch: 200, bookmark: false, ..chapter(5.0, false) }];
        let merged = merge_chapter_state(&old, &[chapter(5.0, false)]);
        assert_eq!((merged[0].date_fetch, merged[0].bookmark), (100, true), "TC-40-022, E-40-81");
    }

    #[test]
    fn an_unrecognised_number_is_left_untouched() {
        let old = [Chapter { date_fetch: 9, bookmark: true, ..chapter(-1.0, true) }];
        let merged = merge_chapter_state(&old, &[Chapter { date_fetch: 1, ..chapter(-1.0, false) }]);
        assert_eq!((merged[0].date_fetch, merged[0].bookmark, merged[0].read), (1, false, false), "TC-40-023: -1 is not matched and not marked");
    }

    #[test]
    fn the_read_flag_is_derived_and_never_copied_from_the_old_chapter() {
        let old = [chapter(9.0, false)];
        let merged = merge_chapter_state(&old, &[chapter(9.0, false)]);
        assert!(!merged[0].read, "TC-40-024: no read point in the old list means nothing is marked");
        let no_old = merge_chapter_state(&[], &[chapter(2.0, false)]);
        assert_eq!((no_old[0].date_fetch, no_old[0].bookmark, no_old[0].read), (0, false, false), "E-40-77: no old chapters copy nothing");
    }
}
