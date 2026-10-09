//! Next-unread chapter and next-unseen episode (`ChapterGetNextUnread.kt`, `EpsiodeGetNextUnseen.kt`, T-52a-007 and
//! T-52a-008). Pure: the caller passes the stored list, the entry, the downloaded-only switch and a download lookup.
//!
//! Order of work as upstream: view filters (unread, bookmarked, downloaded), then the entry's sort, then the first
//! unread of the result, or the last one when the entry sorts descending. The unread filter runs before the search,
//! so "show read only" finds nothing, as upstream.
#![allow(dead_code)] // called by the entry screens' continue button and the player as they land

use std::cmp::Ordering;

use kuro_data::anime::Anime;
use kuro_data::chapter::Chapter;
use kuro_data::episode::Episode;
use kuro_domain::entries::anime::{AnimeExt, episode_flags};
use kuro_domain::entries::manga::{MangaExt, chapter_flags};
use kuro_domain::flags::{TriState, tri};
use kuro_domain::items::sorter::{chapter_cmp, episode_cmp};
use kuro_source::local::LOCAL_SOURCE_ID;

/// The three view filters of an entry, already resolved.
#[derive(Clone, Copy, Debug)]
pub struct ViewFilter {
    pub unread: TriState,
    pub bookmarked: TriState,
    pub downloaded: TriState,
}

/// What the search needs from a chapter or an episode.
pub trait Pending {
    /// `!read` for a chapter, `!seen` for an episode.
    fn is_pending(&self) -> bool;
    fn is_bookmarked(&self) -> bool;
}

impl Pending for Chapter {
    fn is_pending(&self) -> bool {
        !self.read
    }
    fn is_bookmarked(&self) -> bool {
        self.bookmark
    }
}

impl Pending for Episode {
    fn is_pending(&self) -> bool {
        !self.seen
    }
    fn is_bookmarked(&self) -> bool {
        self.bookmark
    }
}

/// The generic search. `is_downloaded` is asked only when the downloaded filter is on; a local entry counts every
/// item as downloaded.
pub fn next_pending<U: Pending + Clone>(
    items: &[U],
    filter: &ViewFilter,
    cmp: impl Fn(&U, &U) -> Ordering,
    descending: bool,
    is_local: bool,
    is_downloaded: impl Fn(&U) -> bool,
) -> Option<U> {
    let mut kept: Vec<U> = items
        .iter()
        .filter(|u| filter.unread.apply_filter(|| u.is_pending()))
        .filter(|u| filter.bookmarked.apply_filter(|| u.is_bookmarked()))
        .filter(|u| filter.downloaded.apply_filter(|| is_local || is_downloaded(u)))
        .cloned()
        .collect();
    kept.sort_by(cmp);
    if descending {
        kept.into_iter().rev().find(|u| u.is_pending())
    } else {
        kept.into_iter().find(|u| u.is_pending())
    }
}

/// `Manga.downloadedFilter`: downloaded-only mode forces `EnabledIs` whatever the entry's own filter says.
fn downloaded_filter(downloaded_only: bool, raw: i64, show: i64, hide: i64) -> TriState {
    if downloaded_only { TriState::ENABLED_IS } else { tri(raw, show, hide) }
}

/// `getNextUnread(manga, chapters)`. `is_downloaded` answers for one chapter of this manga.
pub fn next_unread(
    manga: &kuro_data::manga::Manga,
    chapters: &[Chapter],
    downloaded_only: bool,
    is_downloaded: impl Fn(&Chapter) -> bool,
) -> Option<Chapter> {
    let filter = ViewFilter {
        unread: manga.unread_filter(),
        bookmarked: manga.bookmarked_filter(),
        downloaded: downloaded_filter(
            downloaded_only,
            manga.downloaded_filter_raw(),
            chapter_flags::SHOW_DOWNLOADED,
            chapter_flags::SHOW_NOT_DOWNLOADED,
        ),
    };
    let descending = manga.sort_descending();
    next_pending(
        chapters,
        &filter,
        chapter_cmp(manga.sorting(), descending),
        descending,
        manga.source == LOCAL_SOURCE_ID,
        is_downloaded,
    )
}

/// `getNextUnseen(anime, episodes)`: the same rules over `seen`.
pub fn next_unseen(
    anime: &Anime,
    episodes: &[Episode],
    downloaded_only: bool,
    is_downloaded: impl Fn(&Episode) -> bool,
) -> Option<Episode> {
    let filter = ViewFilter {
        unread: anime.unseen_filter(),
        bookmarked: anime.bookmarked_filter(),
        downloaded: downloaded_filter(
            downloaded_only,
            anime.downloaded_filter_raw(),
            episode_flags::SHOW_DOWNLOADED,
            episode_flags::SHOW_NOT_DOWNLOADED,
        ),
    };
    let descending = anime.sort_descending();
    next_pending(
        episodes,
        &filter,
        episode_cmp(anime.sorting(), descending),
        descending,
        anime.source == LOCAL_SOURCE_ID,
        is_downloaded,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_data::manga::Manga;

    fn chapter(id: i64, number: f64, read: bool) -> Chapter {
        Chapter { id, chapter_number: number, read, source_order: id, ..Chapter::create() }
    }

    fn episode(id: i64, number: f64, seen: bool) -> Episode {
        Episode { id, episode_number: number, seen, source_order: id, ..Episode::create() }
    }

    /// Sorted by chapter number, remote source.
    fn manga(flags: i64) -> Manga {
        Manga { source: 7, chapter_flags: flags | chapter_flags::SORTING_NUMBER, ..Manga::create() }
    }

    fn id_of(c: Option<Chapter>) -> Option<i64> {
        c.map(|c| c.id)
    }

    const ASC: i64 = chapter_flags::SORT_ASC;
    const DESC: i64 = chapter_flags::SORT_DESC;

    #[test]
    fn empty_list_and_all_read_give_none() {
        let m = manga(ASC);
        assert_eq!(id_of(next_unread(&m, &[], false, |_| true)), None);
        let read = [chapter(1, 1.0, true), chapter(2, 2.0, true), chapter(3, 3.0, true)];
        assert_eq!(id_of(next_unread(&m, &read, false, |_| true)), None);
    }

    #[test]
    fn ascending_takes_the_first_unread_and_descending_the_last_fixture_s1() {
        let s1 = [chapter(1, 1.0, true), chapter(2, 2.0, false), chapter(3, 3.0, false)];
        assert_eq!(id_of(next_unread(&manga(ASC), &s1, false, |_| true)), Some(2));
        // sorted descending the list is [3 unread, 2 unread, 1 read]; the last unread is chapter 2
        assert_eq!(id_of(next_unread(&manga(DESC), &s1, false, |_| true)), Some(2));
        let unsorted = [chapter(3, 3.0, false), chapter(1, 1.0, true), chapter(2, 2.0, false)];
        assert_eq!(id_of(next_unread(&manga(ASC), &unsorted, false, |_| true)), Some(2));
    }

    #[test]
    fn show_read_only_with_nothing_unread_matching_returns_none_as_upstream() {
        let m = manga(ASC | chapter_flags::SHOW_READ);
        let list = [chapter(1, 1.0, true), chapter(2, 2.0, true)];
        assert_eq!(id_of(next_unread(&m, &list, false, |_| true)), None);
        let with_unread = [chapter(1, 1.0, true), chapter(2, 2.0, false)];
        assert_eq!(id_of(next_unread(&m, &with_unread, false, |_| true)), None, "the unread one is filtered out first");
    }

    #[test]
    fn downloaded_only_ignores_chapters_that_are_not_downloaded() {
        let m = manga(ASC);
        let list = [chapter(1, 1.0, false), chapter(2, 2.0, false)];
        assert_eq!(id_of(next_unread(&m, &list, true, |_| false)), None);
        assert_eq!(id_of(next_unread(&m, &list, true, |c| c.id == 2)), Some(2));
        assert_eq!(id_of(next_unread(&m, &list, false, |_| false)), Some(1), "off: the lookup is not consulted");
    }

    #[test]
    fn the_entry_own_downloaded_filter_and_a_local_entry() {
        let shown = manga(ASC | chapter_flags::SHOW_DOWNLOADED);
        let list = [chapter(1, 1.0, false), chapter(2, 2.0, false)];
        assert_eq!(id_of(next_unread(&shown, &list, false, |c| c.id == 2)), Some(2));
        let hidden = manga(ASC | chapter_flags::SHOW_NOT_DOWNLOADED);
        assert_eq!(id_of(next_unread(&hidden, &list, false, |c| c.id == 1)), Some(2));
        let local = Manga { source: LOCAL_SOURCE_ID, ..shown };
        assert_eq!(id_of(next_unread(&local, &list, true, |_| false)), Some(1), "every local chapter counts as downloaded");
    }

    #[test]
    fn bookmark_filter_applies_before_the_search() {
        let m = manga(ASC | chapter_flags::SHOW_BOOKMARKED);
        let mut marked = chapter(2, 2.0, false);
        marked.bookmark = true;
        let list = [chapter(1, 1.0, false), marked];
        assert_eq!(id_of(next_unread(&m, &list, false, |_| true)), Some(2));
    }

    #[test]
    fn episodes_follow_the_same_rules_with_seen() {
        let anime = |flags: i64| Anime { source: 7, episode_flags: flags | episode_flags::SORTING_NUMBER, ..Anime::create() };
        let id = |e: Option<Episode>| e.map(|e| e.id);
        let s1 = [episode(1, 1.0, true), episode(2, 2.0, false), episode(3, 3.0, false)];
        assert_eq!(id(next_unseen(&anime(episode_flags::SORT_ASC), &s1, false, |_| true)), Some(2));
        assert_eq!(id(next_unseen(&anime(episode_flags::SORT_DESC), &s1, false, |_| true)), Some(2));
        assert_eq!(id(next_unseen(&anime(episode_flags::SORT_ASC), &[], false, |_| true)), None);
        let all_seen = [episode(1, 1.0, true)];
        assert_eq!(id(next_unseen(&anime(episode_flags::SORT_ASC), &all_seen, false, |_| true)), None);
        assert_eq!(id(next_unseen(&anime(episode_flags::SORT_ASC), &s1, true, |_| false)), None);
        let show_seen = anime(episode_flags::SORT_ASC | episode_flags::SHOW_SEEN);
        assert_eq!(id(next_unseen(&show_seen, &s1, false, |_| true)), None);
    }

    #[test]
    fn the_library_continue_target_agrees_with_the_selection() {
        // player "resume next" and the list's next item both come from one rule: first pending, last when descending
        let list = [chapter(1, 1.0, true), chapter(2, 2.0, false), chapter(3, 3.0, false)];
        let m = manga(ASC);
        let picked = next_unread(&m, &list, false, |_| true).unwrap();
        let continued = kuro_domain::library::active::continue_target(&list, false, |c| !c.read).unwrap();
        assert_eq!(picked.id, continued.id);
    }
}
