//! The per-row download count the library pipeline reads (T-37a-056): the badge, the downloaded filter and the global
//! downloaded-only switch all start from it. The count comes from the shell's download index, which is the manga
//! download cache; the app has no anime download cache yet, so anime rows count 0 until one is wired.

use std::sync::Arc;

use kuro_platform::kind::Kind;
use kuro_domain::library::model::LibraryRow;
use kuro_downloads::index::DownloadIndex;

/// How many chapters of the row's entry are on disk. A local entry has no download folder of its own (its files are
/// the entry), so it counts 0 here and the filter keeps it through its own local rule.
pub fn count(kind: Kind, index: Option<&DownloadIndex>, row: &LibraryRow) -> i64 {
    match (kind, index) {
        (Kind::Manga, Some(index)) if !row.is_local() => index.chapter_count_manga(row.source, &row.title) as i64,
        _ => 0,
    }
}

/// The lookup `Facts::downloads` holds. It owns a clone of the index (cheap) so the worker thread can call it.
pub fn lookup(kind: Kind, index: Option<DownloadIndex>) -> Arc<dyn Fn(&LibraryRow) -> i64 + Send + Sync> {
    Arc::new(move |row| count(kind, index.as_ref(), row))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(source: i64, title: &str) -> LibraryRow {
        LibraryRow {
            id: 1,
            source,
            title: title.into(),
            category: 0,
            total: 3,
            done: 0,
            bookmarks: 0,
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
    fn the_count_is_the_chapters_the_index_holds_for_the_source_and_title() {
        let dir = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(dir.path(), dir.path());
        index.add_chapter(7, "Source", "Berserk", "Ch 1");
        index.add_chapter(7, "Source", "Berserk", "Ch 2");
        let count = lookup(Kind::Manga, Some(index));
        assert_eq!(count(&row(7, "Berserk")), 2);
        assert_eq!(count(&row(7, "Other")), 0);
        assert_eq!(count(&row(8, "Berserk")), 0, "another source's folder is not this entry's");
    }

    #[test]
    fn a_new_download_shows_without_rebuilding_the_lookup() {
        let dir = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(dir.path(), dir.path());
        let count = lookup(Kind::Manga, Some(index.clone()));
        assert_eq!(count(&row(7, "Berserk")), 0);
        index.add_chapter(7, "Source", "Berserk", "Ch 1");
        assert_eq!(count(&row(7, "Berserk")), 1);
    }

    #[test]
    fn no_index_or_an_anime_row_counts_zero() {
        let dir = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(dir.path(), dir.path());
        index.add_chapter(7, "Source", "Berserk", "Ch 1");
        assert_eq!(lookup(Kind::Manga, None)(&row(7, "Berserk")), 0);
        assert_eq!(lookup(Kind::Anime, Some(index))(&row(7, "Berserk")), 0);
    }

    #[test]
    fn a_local_row_counts_zero() {
        let dir = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(dir.path(), dir.path());
        index.add_chapter(0, "Local", "Berserk", "Ch 1");
        assert_eq!(lookup(Kind::Manga, Some(index))(&row(0, "Berserk")), 0);
    }
}
