//! The downloaded-only chapter helper (T-38b-007). Upstream: `List<Chapter>.filterDownloadedChapters(manga)`. The reader's
//! skip paths ask the same question of a chapter (`Fn(&Chapter) -> bool`), so `downloaded_lookup` has the shape of the
//! reader's `is_downloaded` argument. A local manga counts every chapter as downloaded and is never looked up.

use kuro_data::chapter::Chapter;
use kuro_downloads::index::DownloadIndex;

/// The chapters that are on disk. A local manga's chapters come back unchanged.
pub fn filter_downloaded_chapters(chapters: Vec<Chapter>, local: bool, is_downloaded: impl Fn(&Chapter) -> bool) -> Vec<Chapter> {
    if local {
        return chapters;
    }
    chapters.into_iter().filter(|c| is_downloaded(c)).collect()
}

/// The lookup of one manga over the download index, with the local passthrough: it is true for every chapter of a local
/// manga and otherwise asks the index for the chapter's name and scanlator.
pub fn downloaded_lookup(index: DownloadIndex, source_id: i64, manga_title: String, local: bool) -> impl Fn(&Chapter) -> bool {
    move |chapter| local || index.is_chapter_downloaded(source_id, &manga_title, &chapter.name, chapter.scanlator.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(id: i64, name: &str, scanlator: Option<&str>) -> Chapter {
        Chapter { id, name: name.into(), scanlator: scanlator.map(str::to_owned), ..Chapter::create() }
    }

    fn ids(chapters: &[Chapter]) -> Vec<i64> {
        chapters.iter().map(|c| c.id).collect()
    }

    #[test]
    fn a_local_manga_returns_its_chapters_unchanged_and_never_asks() {
        let all = vec![chapter(1, "Ch 1", None), chapter(2, "Ch 2", None)];
        let kept = filter_downloaded_chapters(all, true, |_| panic!("a local manga is not looked up"));
        assert_eq!(ids(&kept), vec![1, 2]);
    }

    #[test]
    fn another_manga_keeps_only_the_chapters_that_are_downloaded() {
        let all = vec![chapter(1, "Ch 1", None), chapter(2, "Ch 2", None), chapter(3, "Ch 3", None)];
        let kept = filter_downloaded_chapters(all, false, |c| c.id != 2);
        assert_eq!(ids(&kept), vec![1, 3]);
    }

    #[test]
    fn the_index_lookup_matches_the_chapter_folder_and_a_local_manga_passes_everything() {
        let dir = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(dir.path(), dir.path());
        index.add_chapter(7, "Source (EN)", "Berserk", "Ch 1");
        let (one, two) = (chapter(1, "Ch 1", None), chapter(2, "Ch 2", None));
        let lookup = downloaded_lookup(index.clone(), 7, "Berserk".into(), false);
        assert!(lookup(&one) && !lookup(&two));
        let local = downloaded_lookup(index, 7, "Berserk".into(), true);
        assert!(local(&one) && local(&two));
    }
}
