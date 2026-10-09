//! The reader's chapter list: filters, the current chapter re-append, skip-duplicates, downloaded-only, and the
//! sort (`ReaderViewModel.kt:154-223`, `ChapterRemoveDuplicates.kt`, `ChapterFilterDownloaded.kt`). T-43a-018,
//! T-43a-019, T-43a-020. The gap rule (T-43a-021) is `kuro_domain::items::missing`, used by the transition card.

use kuro_data::chapter::Chapter;
use kuro_domain::entries::manga::chapter_flags::SORTING_MASK;
use kuro_domain::items::sorter::chapter_cmp;
use kuro_prefs::flags::{
    CHAPTER_BOOKMARKED_MASK, CHAPTER_DOWNLOADED_MASK, CHAPTER_SHOW_BOOKMARKED, CHAPTER_SHOW_DOWNLOADED,
    CHAPTER_SHOW_NOT_BOOKMARKED, CHAPTER_SHOW_NOT_DOWNLOADED, CHAPTER_SHOW_READ, CHAPTER_SHOW_UNREAD,
    CHAPTER_UNREAD_MASK,
};

/// The preferences and manga flags the list depends on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListOptions {
    /// `skip_read`: drop read chapters.
    pub skip_read: bool,
    /// `skip_filtered`: apply the manga's unread, downloaded and bookmarked filters.
    pub skip_filtered: bool,
    /// `skip_dupe`: keep one chapter per chapter number.
    pub skip_dupe: bool,
    /// Downloaded-only mode (`basePreferences.downloadedOnly`).
    pub downloaded_only: bool,
    /// The manga's `chapter_flags`, which hold the three filters and the sort.
    pub chapter_flags: i64,
}

/// Why the list could not be built. `ChapterNotFound` is the upstream `error(...)` at `ReaderViewModel.kt:158`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListError {
    ChapterNotFound(i64),
    /// Downloaded-only removed the requested chapter (E-LIST-2).
    CurrentChapterMissing,
}

/// The removal predicate (`when` with three arms, in order). `true` removes the chapter.
pub fn is_filtered_out(chapter: &Chapter, opts: &ListOptions, is_downloaded: impl Fn(&Chapter) -> bool) -> bool {
    if opts.skip_read && chapter.read {
        return true;
    }
    if !opts.skip_filtered {
        return false;
    }
    let flags = opts.chapter_flags;
    let unread = flags & CHAPTER_UNREAD_MASK;
    let downloaded = flags & CHAPTER_DOWNLOADED_MASK;
    let bookmarked = flags & CHAPTER_BOOKMARKED_MASK;
    (unread == CHAPTER_SHOW_READ && !chapter.read)
        || (unread == CHAPTER_SHOW_UNREAD && chapter.read)
        || (downloaded == CHAPTER_SHOW_DOWNLOADED && !is_downloaded(chapter))
        || (downloaded == CHAPTER_SHOW_NOT_DOWNLOADED && is_downloaded(chapter))
        || (bookmarked == CHAPTER_SHOW_BOOKMARKED && !chapter.bookmark)
        || (bookmarked == CHAPTER_SHOW_NOT_BOOKMARKED && chapter.bookmark)
}

/// `removeDuplicates(currentChapter)`: groups by chapter number in first-appearance order and keeps, per group,
/// the current chapter, else the same scanlator, else the first. Unknown numbers (`-1`) form one group.
pub fn remove_duplicates(chapters: &[Chapter], current: &Chapter) -> Vec<Chapter> {
    let mut groups: Vec<(f64, Vec<&Chapter>)> = Vec::new();
    for chapter in chapters {
        match groups.iter_mut().find(|(number, _)| *number == chapter.chapter_number) {
            Some((_, members)) => members.push(chapter),
            None => groups.push((chapter.chapter_number, vec![chapter])),
        }
    }
    groups
        .into_iter()
        .filter_map(|(_, members)| {
            members
                .iter()
                .find(|c| c.id == current.id)
                .or_else(|| members.iter().find(|c| c.scanlator == current.scanlator))
                .or_else(|| members.first())
                .map(|c| (*c).clone())
        })
        .collect()
}

/// The reader list for a manga. `chapters` is the manga's list (already scanlator-filtered by the repository),
/// `current_id` the requested chapter, `is_downloaded` the download lookup (`skipCache` false).
pub fn build_reader_list(
    chapters: &[Chapter],
    current_id: i64,
    opts: &ListOptions,
    is_downloaded: impl Fn(&Chapter) -> bool,
) -> Result<Vec<Chapter>, ListError> {
    let selected = chapters
        .iter()
        .find(|c| c.id == current_id)
        .cloned()
        .ok_or(ListError::ChapterNotFound(current_id))?;

    let mut list: Vec<Chapter> = if opts.skip_read || opts.skip_filtered {
        let mut kept: Vec<Chapter> =
            chapters.iter().filter(|c| !is_filtered_out(c, opts, &is_downloaded)).cloned().collect();
        // the current chapter stays reachable even when a filter removed it (E-LIST-1)
        if !kept.iter().any(|c| c.id == current_id) {
            kept.push(selected.clone());
        }
        kept
    } else {
        chapters.to_vec()
    };

    list.sort_by(chapter_cmp(opts.chapter_flags & SORTING_MASK, false));

    if opts.skip_dupe {
        list = remove_duplicates(&list, &selected);
    }

    if opts.downloaded_only {
        list.retain(|c| is_downloaded(c));
        if !list.iter().any(|c| c.id == current_id) {
            return Err(ListError::CurrentChapterMissing);
        }
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::flags::CHAPTER_SHOW_NOT_DOWNLOADED;

    /// A chapter with the given id, number, read flag and scanlator. `source_order` follows the id.
    fn ch(id: i64, number: f64, read: bool, scanlator: Option<&str>) -> Chapter {
        let mut c = Chapter::create();
        c.id = id;
        c.url = format!("/c/{id}");
        c.name = format!("Chapter {number}");
        c.chapter_number = number;
        c.read = read;
        c.source_order = id;
        c.scanlator = scanlator.map(String::from);
        c
    }

    fn opts(flags: i64) -> ListOptions {
        ListOptions { skip_read: false, skip_filtered: false, skip_dupe: false, downloaded_only: false, chapter_flags: flags }
    }

    fn ids(list: &[Chapter]) -> Vec<i64> {
        list.iter().map(|c| c.id).collect()
    }

    const NO_DOWNLOADS: fn(&Chapter) -> bool = |_| false;

    #[test]
    fn list_skip_read_removes_read_chapters_only() {
        let chapters = [ch(1, 1.0, true, None), ch(2, 2.0, false, None), ch(3, 3.0, true, None)];
        let list = build_reader_list(&chapters, 2, &ListOptions { skip_read: true, ..opts(0) }, NO_DOWNLOADS).unwrap();
        assert_eq!(ids(&list), [2]);
    }

    #[test]
    fn list_skip_filtered_applies_the_unread_filter() {
        let chapters = [ch(1, 1.0, true, None), ch(2, 2.0, false, None), ch(3, 3.0, true, None)];
        let flags = CHAPTER_SHOW_UNREAD;
        let list = build_reader_list(&chapters, 2, &ListOptions { skip_filtered: true, ..opts(flags) }, NO_DOWNLOADS)
            .unwrap();
        assert_eq!(ids(&list), [2], "read chapters are removed by the show-unread filter");
    }

    #[test]
    fn list_skip_filtered_keeps_the_current_chapter_even_when_filtered_out() {
        let chapters = [ch(1, 1.0, false, None), ch(2, 2.0, true, None), ch(3, 3.0, false, None)];
        let flags = CHAPTER_SHOW_UNREAD;
        let list = build_reader_list(&chapters, 2, &ListOptions { skip_filtered: true, ..opts(flags) }, NO_DOWNLOADS)
            .unwrap();
        assert_eq!(ids(&list), [3, 2, 1], "the removed current chapter is re-appended, then sorted by source order");
    }

    #[test]
    fn list_skip_filtered_downloaded_and_bookmarked_arms() {
        let mut bookmarked = ch(2, 2.0, false, None);
        bookmarked.bookmark = true;
        let chapters = [ch(1, 1.0, false, None), bookmarked, ch(3, 3.0, false, None)];
        let not_bookmarked = build_reader_list(
            &chapters,
            1,
            &ListOptions { skip_filtered: true, ..opts(0x100 | CHAPTER_SHOW_NOT_BOOKMARKED) },
            NO_DOWNLOADS,
        )
        .unwrap();
        assert_eq!(ids(&not_bookmarked), [1, 3]);

        let downloaded_only_filter = build_reader_list(
            &chapters,
            1,
            &ListOptions { skip_filtered: true, ..opts(0x100 | CHAPTER_SHOW_NOT_DOWNLOADED) },
            |c| c.id == 3,
        )
        .unwrap();
        assert_eq!(ids(&downloaded_only_filter), [1, 2]);
    }

    #[test]
    fn list_dedupe_keeps_current_id_then_scanlator_then_first() {
        let chapters = [
            ch(10, 1.0, false, Some("A")),
            ch(11, 1.0, false, Some("B")),
            ch(12, 2.0, false, Some("A")),
            ch(13, 2.0, false, Some("B")),
            ch(14, 2.0, false, Some("C")),
        ];
        let list =
            build_reader_list(&chapters, 11, &ListOptions { skip_dupe: true, ..opts(0x100) }, NO_DOWNLOADS).unwrap();
        assert_eq!(ids(&list), [11, 13], "group 1 keeps the current id; group 2 has no current id, so the same scanlator (B) wins");
        let missing = build_reader_list(&chapters, 99, &ListOptions { skip_dupe: true, ..opts(0) }, NO_DOWNLOADS);
        assert_eq!(missing.err(), Some(ListError::ChapterNotFound(99)));
    }

    #[test]
    fn all_unknown_numbers_collapse_into_one_entry_with_dedupe() {
        let chapters = [ch(1, -1.0, false, None), ch(2, -1.0, false, None), ch(3, 4.0, false, None)];
        let list =
            build_reader_list(&chapters, 1, &ListOptions { skip_dupe: true, ..opts(0x100) }, NO_DOWNLOADS).unwrap();
        assert_eq!(ids(&list), [1, 3]);
    }

    #[test]
    fn downloaded_only_removes_the_requested_chapter_when_not_downloaded() {
        let chapters = [ch(1, 1.0, false, None), ch(2, 2.0, false, None)];
        let result = build_reader_list(
            &chapters,
            2,
            &ListOptions { downloaded_only: true, ..opts(0) },
            |c| c.id == 1,
        );
        assert_eq!(result, Err(ListError::CurrentChapterMissing));
        let ok = build_reader_list(&chapters, 1, &ListOptions { downloaded_only: true, ..opts(0) }, |c| c.id == 1);
        assert_eq!(ids(&ok.unwrap()), [1]);
    }

    #[test]
    fn sort_is_ascending_for_number_and_reversed_for_source() {
        let chapters = [ch(1, 3.0, false, None), ch(2, -1.0, false, None), ch(3, 1.0, false, None)];
        let by_number = build_reader_list(&chapters, 1, &opts(0x100), NO_DOWNLOADS).unwrap();
        assert_eq!(ids(&by_number), [2, 3, 1], "unknown numbers sort before known ones");
        let by_source = build_reader_list(&chapters, 1, &opts(0x0), NO_DOWNLOADS).unwrap();
        assert_eq!(ids(&by_source), [3, 2, 1], "source order reads oldest first (descending source_order)");
    }

    #[test]
    fn list_order_is_ascending_for_every_mode() {
        // three chapters whose number, upload date and name all put id 2 first, then 3, then 1
        let mut chapters = [ch(1, 3.0, false, None), ch(2, 1.0, false, None), ch(3, 2.0, false, None)];
        for (c, (date, name)) in chapters.iter_mut().zip([(300, "c"), (100, "a"), (200, "b")]) {
            c.date_upload = date;
            c.name = name.to_owned();
        }
        let expected = [(0x100, [2, 3, 1]), (0x200, [2, 3, 1]), (0x300, [2, 3, 1]), (0x0, [3, 2, 1])];
        for (mode, order) in expected {
            // the manga's own direction bit never reverses the reader list
            for direction in [0x0, 0x1] {
                let list = build_reader_list(&chapters, 1, &opts(mode | direction), NO_DOWNLOADS).unwrap();
                assert_eq!(ids(&list), order, "sorting {mode:#x} direction {direction:#x}");
            }
        }
    }

    #[test]
    fn remove_duplicates_is_a_pure_grouping_over_the_sorted_input() {
        let chapters = [ch(1, 1.0, false, Some("A")), ch(2, 1.0, false, Some("B")), ch(3, 2.0, false, Some("B"))];
        let current = ch(2, 1.0, false, Some("B"));
        let out = remove_duplicates(&chapters, &current);
        assert_eq!(ids(&out), [2, 3]);
    }

    #[test]
    fn missing_chapters_gap_vectors_hold_on_the_domain_rule() {
        use kuro_domain::items::missing::{gap, missing_items_count};
        assert_eq!(gap(10.0, 9.0), 0);
        assert_eq!(gap(10.0, 8.0), 1);
        assert_eq!(gap(10.0, 8.5), 1);
        assert_eq!(gap(10.0, 1.1), 8);
        assert_eq!(gap(-1.0, 3.0), 0);
        assert_eq!(missing_items_count(&[-1.0, 1.0, 2.0, 2.2, 4.0, 6.0, 10.0, 10.0]), 5);
    }
}
