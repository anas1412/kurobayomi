//! Next-unread chapter and next-unseen episode (`ChapterGetNextUnread.kt`, `EpsiodeGetNextUnseen.kt`, T-52a-007 and
//! T-52a-008): one rule for both kinds over an `Entry` and its `Item`s. Pure: the caller passes the stored list, the
//! entry, the downloaded-only switch and a download lookup.
//!
//! Order of work as upstream: view filters (unread, bookmarked, downloaded), then the entry's sort, then the first
//! unread of the result, or the last one when the entry sorts descending. The unread filter runs before the search,
//! so "show read only" finds nothing, as upstream. The fillermark filter is not part of the search (as before).

use kuro_data::entry::Entry;
use kuro_data::item::Item;
use kuro_domain::app::items::{ListFilters, apply_filters, item_filters};
use kuro_domain::error::DomainError;
use kuro_domain::library::active::continue_target;
use kuro_prefs::tristate::TriState;
use kuro_source::local::LOCAL_SOURCE_ID;

/// `getNextUnread(manga, chapters)` and `getNextUnseen(anime, episodes)`. `is_downloaded` answers for one item of this
/// entry and is asked only when the downloaded filter is on; a local entry counts every item as downloaded.
pub fn next_pending(
    entry: &Entry,
    items: Vec<Item>,
    downloaded_only: bool,
    is_downloaded: impl Fn(&Item) -> bool,
) -> Result<Option<Item>, DomainError> {
    let filters = ListFilters { fillermarked: TriState::DISABLED, ..item_filters(entry, downloaded_only)? };
    let kept = apply_filters(items, &filters, entry.source == LOCAL_SOURCE_ID, is_downloaded);
    Ok(continue_target(&kept, filters.descending, |i| !i.done).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_db::flags::item::{SHOW_BOOKMARKED, SHOW_DONE, SHOW_DOWNLOADED, SHOW_NOT_DOWNLOADED, SHOW_UNDONE, SORTING_NUMBER, SORT_ASC, SORT_DESC};
    use kuro_platform::kind::Kind;

    fn item(kind: Kind, id: i64, number: f64, done: bool) -> Item {
        Item { id, number, done, source_order: id, ..Item::create(kind) }
    }

    /// Sorted by number, remote source.
    fn entry(kind: Kind, flags: i64) -> Entry {
        Entry { source: 7, item_flags: flags | SORTING_NUMBER, ..Entry::create(kind) }
    }

    fn next(e: &Entry, items: Vec<Item>, downloaded_only: bool, is_downloaded: impl Fn(&Item) -> bool) -> Option<i64> {
        next_pending(e, items, downloaded_only, is_downloaded).expect("known sorting").map(|i| i.id)
    }

    #[test]
    fn empty_list_and_all_done_give_none() {
        for kind in [Kind::Manga, Kind::Anime] {
            let e = entry(kind, SORT_ASC);
            assert_eq!(next(&e, vec![], false, |_| true), None);
            let done = vec![item(kind, 1, 1.0, true), item(kind, 2, 2.0, true), item(kind, 3, 3.0, true)];
            assert_eq!(next(&e, done, false, |_| true), None);
        }
    }

    #[test]
    fn ascending_takes_the_first_unread_and_descending_the_last_fixture_s1() {
        for kind in [Kind::Manga, Kind::Anime] {
            let s1 = || vec![item(kind, 1, 1.0, true), item(kind, 2, 2.0, false), item(kind, 3, 3.0, false)];
            assert_eq!(next(&entry(kind, SORT_ASC), s1(), false, |_| true), Some(2));
            // sorted descending the list is [3 unread, 2 unread, 1 read]; the last unread is item 2
            assert_eq!(next(&entry(kind, SORT_DESC), s1(), false, |_| true), Some(2));
            let unsorted = vec![item(kind, 3, 3.0, false), item(kind, 1, 1.0, true), item(kind, 2, 2.0, false)];
            assert_eq!(next(&entry(kind, SORT_ASC), unsorted, false, |_| true), Some(2));
        }
    }

    #[test]
    fn show_done_only_with_nothing_unread_matching_returns_none_as_upstream() {
        for kind in [Kind::Manga, Kind::Anime] {
            let e = entry(kind, SORT_ASC | SHOW_DONE);
            let list = vec![item(kind, 1, 1.0, true), item(kind, 2, 2.0, true)];
            assert_eq!(next(&e, list, false, |_| true), None);
            let with_unread = vec![item(kind, 1, 1.0, true), item(kind, 2, 2.0, false)];
            assert_eq!(next(&e, with_unread, false, |_| true), None, "the unread one is filtered out first");
        }
    }

    #[test]
    fn show_undone_keeps_only_unread_rows() {
        let e = entry(Kind::Anime, SORT_ASC | SHOW_UNDONE);
        let list = vec![item(Kind::Anime, 1, 1.0, true), item(Kind::Anime, 2, 2.0, false)];
        assert_eq!(next(&e, list, false, |_| true), Some(2));
    }

    #[test]
    fn downloaded_only_ignores_items_that_are_not_downloaded() {
        let e = entry(Kind::Manga, SORT_ASC);
        let list = || vec![item(Kind::Manga, 1, 1.0, false), item(Kind::Manga, 2, 2.0, false)];
        assert_eq!(next(&e, list(), true, |_| false), None);
        assert_eq!(next(&e, list(), true, |i| i.id == 2), Some(2));
        assert_eq!(next(&e, list(), false, |_| false), Some(1), "off: the lookup is not consulted");
    }

    #[test]
    fn the_entry_own_downloaded_filter_and_a_local_entry() {
        let shown = entry(Kind::Manga, SORT_ASC | SHOW_DOWNLOADED);
        let list = || vec![item(Kind::Manga, 1, 1.0, false), item(Kind::Manga, 2, 2.0, false)];
        assert_eq!(next(&shown, list(), false, |i| i.id == 2), Some(2));
        let hidden = entry(Kind::Manga, SORT_ASC | SHOW_NOT_DOWNLOADED);
        assert_eq!(next(&hidden, list(), false, |i| i.id == 1), Some(2));
        let local = Entry { source: LOCAL_SOURCE_ID, ..shown };
        assert_eq!(next(&local, list(), true, |_| false), Some(1), "every local item counts as downloaded");
    }

    #[test]
    fn bookmark_filter_applies_before_the_search() {
        let e = entry(Kind::Manga, SORT_ASC | SHOW_BOOKMARKED);
        let marked = Item { bookmark: true, ..item(Kind::Manga, 2, 2.0, false) };
        assert_eq!(next(&e, vec![item(Kind::Manga, 1, 1.0, false), marked], false, |_| true), Some(2));
    }

    #[test]
    fn the_library_continue_target_agrees_with_the_selection() {
        // player "resume next" and the list's next item both come from one rule: first pending, last when descending
        let list = vec![item(Kind::Manga, 1, 1.0, true), item(Kind::Manga, 2, 2.0, false), item(Kind::Manga, 3, 3.0, false)];
        let picked = next(&entry(Kind::Manga, SORT_ASC), list.clone(), false, |_| true).unwrap();
        let continued = continue_target(&list, false, |i| !i.done).unwrap();
        assert_eq!(picked, continued.id);
    }
}
