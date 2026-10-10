//! Download-ahead while reading (T-43a-034, E-DLA-1, E-DLA-2). While a downloaded chapter is read, the chapters after it
//! are queued for download. The rules are pure and tested here. The session applies them on every page change and hands
//! the chapter ids to the sink of `ReaderExtras::download_ahead`; the app has no download manager to give that sink yet,
//! so until it does the ids go nowhere.

/// How many chapters to queue now: zero unless the amount is positive, the current and next chapters are both downloaded,
/// and the page is past a quarter of the chapter (the 1-based page number, `page * 4 > pages`).
pub fn ahead_amount(amount: i32, current_downloaded: bool, next_downloaded: bool, page: usize, pages: usize) -> usize {
    let past_quarter = page * 4 > pages;
    if amount <= 0 || !current_downloaded || !next_downloaded || !past_quarter {
        return 0;
    }
    amount as usize
}

/// The chapter ids to queue: at most `amount` of them, starting at `next` (the index of the next chapter) in list order.
pub fn queue_from_next(chapter_ids: &[i64], next: usize, amount: usize) -> &[i64] {
    let start = next.min(chapter_ids.len());
    let end = start.saturating_add(amount).min(chapter_ids.len());
    &chapter_ids[start..end]
}

/// The chapter ids to queue for the page the reader is on. `current` is the index of the open chapter in `chapter_ids`,
/// `page` is 1-based, and `downloaded` answers for a list position. The next chapter is the one after `current`.
pub fn chapters_to_queue(
    amount: i32,
    chapter_ids: &[i64],
    downloaded: impl Fn(usize) -> bool,
    current: usize,
    page: usize,
    pages: usize,
) -> Vec<i64> {
    let next = current + 1;
    if next >= chapter_ids.len() {
        return Vec::new();
    }
    let count = ahead_amount(amount, downloaded(current), downloaded(next), page, pages);
    queue_from_next(chapter_ids, next, count).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_queued_unless_every_condition_holds() {
        assert_eq!(ahead_amount(0, true, true, 10, 20), 0, "amount zero is off");
        assert_eq!(ahead_amount(-2, true, true, 10, 20), 0, "a negative amount is off");
        assert_eq!(ahead_amount(3, false, true, 10, 20), 0, "the current chapter must be downloaded");
        assert_eq!(ahead_amount(3, true, false, 10, 20), 0, "the next chapter must be downloaded");
    }

    #[test]
    fn the_page_must_be_past_a_quarter_of_the_chapter() {
        assert_eq!(ahead_amount(3, true, true, 5, 20), 0, "exactly a quarter is not past it");
        assert_eq!(ahead_amount(3, true, true, 6, 20), 3, "page 6 of 20 is past a quarter");
    }

    #[test]
    fn the_queue_starts_at_the_next_chapter_and_holds_at_most_the_amount() {
        let ids = [11, 12, 13, 14];
        assert_eq!(queue_from_next(&ids, 1, 2), [12, 13]);
        assert_eq!(queue_from_next(&ids, 3, 5), [14], "the list ends before the amount does");
        assert!(queue_from_next(&ids, 9, 2).is_empty(), "a next index past the list queues nothing");
    }

    #[test]
    fn the_plan_queues_from_the_chapter_after_the_open_one() {
        let ids = [11, 12, 13, 14];
        assert_eq!(chapters_to_queue(2, &ids, |_| true, 0, 6, 20), [12, 13]);
        assert!(chapters_to_queue(2, &ids, |i| i != 1, 0, 6, 20).is_empty(), "the next chapter is not downloaded");
        assert!(chapters_to_queue(2, &ids, |_| true, 3, 6, 20).is_empty(), "the open chapter is the last");
        assert!(chapters_to_queue(2, &ids, |_| true, 0, 5, 20).is_empty(), "not past a quarter");
    }
}
