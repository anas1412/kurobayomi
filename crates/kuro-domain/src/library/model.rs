//! The library row: one favourite entry in one category, with the counts the library shows. Both kinds map to
//! this shape (`LibraryManga` / `LibraryAnime`), so the page builder and the sorts never branch on kind.

use kuro_data::anime::LibraryAnime;
use kuro_data::manga::LibraryManga;

/// One `libraryView` / `animelibView` row. `category` is 0 for an entry in no category.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryRow {
    pub id: i64,
    pub source: i64,
    pub title: String,
    pub category: i64,
    /// Chapters (or episodes, or child seasons for a season parent) that count, after excluded scanlators.
    pub total: i64,
    /// Read chapters (seen episodes, or fully seen child seasons).
    pub done: i64,
    pub bookmarks: i64,
    /// Always 0 for manga.
    pub fillermarks: i64,
    pub latest_upload: i64,
    pub item_fetched_at: i64,
    pub last_activity: i64,
    pub last_update: i64,
    pub date_added: i64,
    /// `Manga.status` / `Anime.status`: `SManga.COMPLETED` is 2.
    pub status: i64,
    /// `fetch_interval`; a negative value marks a customised interval (`filterFnIntervalCustom`).
    pub fetch_interval: i32,
    /// `viewer_flags`; anime carries the airing time in bits 24 to 55, manga is 0.
    pub viewer_flags: i64,
    /// `update_strategy == AlwaysUpdate` (the only strategy the library update job refreshes).
    pub always_update: bool,
    /// `next_update` in epoch ms, compared with the fetch window (`ENTRY_OUTSIDE_RELEASE_PERIOD`).
    pub next_update: i64,
}

impl LibraryRow {
    /// Items not yet read or seen (`unreadCount` / `unseenCount`).
    pub fn pending(&self) -> i64 {
        self.total - self.done
    }

    pub fn has_started(&self) -> bool {
        self.done > 0
    }

    pub fn has_bookmarks(&self) -> bool {
        self.bookmarks > 0
    }

    /// A local entry is the one from source 0 (the Local source id).
    pub fn is_local(&self) -> bool {
        self.source == 0
    }

    pub fn from_manga(l: &LibraryManga) -> Self {
        Self {
            id: l.manga.id,
            source: l.manga.source,
            title: l.manga.title.clone(),
            category: l.category,
            total: l.total_chapters,
            done: l.read_count,
            bookmarks: l.bookmark_count,
            fillermarks: 0,
            latest_upload: l.latest_upload,
            item_fetched_at: l.chapter_fetched_at,
            last_activity: l.last_read,
            last_update: l.manga.last_update,
            date_added: l.manga.date_added,
            status: l.manga.status,
            fetch_interval: l.manga.fetch_interval,
            viewer_flags: 0,
            always_update: l.manga.update_strategy == kuro_db::codecs::UpdateStrategy::AlwaysUpdate,
            next_update: l.manga.next_update,
        }
    }

    pub fn from_anime(l: &LibraryAnime) -> Self {
        Self {
            id: l.anime.id,
            source: l.anime.source,
            title: l.anime.title.clone(),
            category: l.category,
            total: l.total_count,
            done: l.seen_count,
            bookmarks: l.bookmark_count,
            fillermarks: l.fillermark_count,
            latest_upload: l.latest_upload,
            item_fetched_at: l.episode_fetched_at,
            last_activity: l.last_seen,
            last_update: l.anime.last_update,
            date_added: l.anime.date_added,
            status: l.anime.status,
            fetch_interval: l.anime.fetch_interval,
            viewer_flags: l.anime.viewer_flags,
            always_update: l.anime.update_strategy == kuro_db::codecs::UpdateStrategy::AlwaysUpdate,
            next_update: l.anime.next_update,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(total: i64, done: i64, bookmarks: i64) -> LibraryRow {
        LibraryRow {
            id: 1,
            source: 7,
            title: "T".into(),
            category: 0,
            total,
            done,
            bookmarks,
            fillermarks: 0,
            latest_upload: 0,
            item_fetched_at: 0,
            last_activity: 0,
            last_update: 0,
            date_added: 0,
            status: 0,
            fetch_interval: 0,
            viewer_flags: 0,
            always_update: true,
            next_update: 0,
        }
    }

    #[test]
    fn derived_values_match_the_library_row_definitions() {
        let r = row(10, 4, 2);
        assert_eq!(r.pending(), 6);
        assert!(r.has_started());
        assert!(r.has_bookmarks());
        let none = row(3, 0, 0);
        assert_eq!(none.pending(), 3);
        assert!(!none.has_started());
        assert!(!none.has_bookmarks());
        assert!(!r.is_local());
    }

    #[test]
    fn source_zero_is_local() {
        let mut r = row(1, 0, 0);
        r.source = 0;
        assert!(r.is_local());
    }
}
