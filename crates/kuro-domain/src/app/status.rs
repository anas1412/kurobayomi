//! Read and seen status of a list (`SetReadStatus.kt`, `SetSeenStatus.kt`; T-10a-013, T-10a-014). The service is
//! [`crate::items::status::SetDoneStatus`]; this file keeps the four pure functions the Library uses, over the legacy
//! chapter and episode shapes, as views of the shared rule. Phase E deletes it.

use kuro_data::chapter::{Chapter, ChapterUpdate};
use kuro_data::episode::{Episode, EpisodeUpdate};
use kuro_data::item::Item;

use crate::items::status::{done_update, items_to_change};

/// The chapters a mark-read or mark-unread touches (`SetReadStatus.await`, step 1). Marking read takes unread rows;
/// marking unread takes rows that are read or have a page position to reset.
pub fn chapters_to_change(read: bool, chapters: &[Chapter]) -> Vec<Chapter> {
    let items: Vec<Item> = chapters.iter().map(Item::from).collect();
    items_to_change(read, &items).into_iter().map(Chapter::from).collect()
}

/// The update of one chapter (step 2). Marking read keeps `last_page_read` (None); marking unread resets it to 0 (EC-26).
pub fn chapter_status_update(read: bool, chapter: &Chapter) -> ChapterUpdate {
    let u = done_update(read, &Item::from(chapter));
    ChapterUpdate { read: u.done, last_page_read: u.progress, ..ChapterUpdate::new(chapter.id) }
}

/// The episodes a mark-seen or mark-unseen touches. Same rule as chapters, on `seen` and `last_second_seen`.
pub fn episodes_to_change(seen: bool, episodes: &[Episode]) -> Vec<Episode> {
    let items: Vec<Item> = episodes.iter().map(Item::from).collect();
    items_to_change(seen, &items).into_iter().map(Episode::from).collect()
}

/// The update of one episode. Marking unseen resets `last_second_seen` to 0; `total_seconds` is never touched.
pub fn episode_status_update(seen: bool, episode: &Episode) -> EpisodeUpdate {
    let u = done_update(seen, &Item::from(episode));
    EpisodeUpdate { seen: u.done, last_second_seen: u.progress, ..EpisodeUpdate::new(episode.id) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(read: bool, last_page_read: i64, number: f64) -> Chapter {
        Chapter { read, last_page_read, chapter_number: number, ..Chapter::create() }
    }

    fn episode(seen: bool, last_second_seen: i64, number: f64) -> Episode {
        Episode { seen, last_second_seen, episode_number: number, ..Episode::create() }
    }

    #[test]
    fn marking_read_takes_only_unread_and_keeps_the_page() {
        let rows = [chapter(false, 0, 1.0), chapter(true, 0, 2.0), chapter(false, 5, 3.0)];
        let targets = chapters_to_change(true, &rows);
        assert_eq!(targets.iter().map(|c| c.chapter_number).collect::<Vec<_>>(), [1.0, 3.0]);
        assert_eq!(chapter_status_update(true, &targets[1]).last_page_read, None, "last_page_read untouched");
    }

    #[test]
    fn marking_unread_takes_read_rows_and_rows_with_a_page_and_resets_the_page() {
        let rows = [chapter(false, 0, 1.0), chapter(true, 0, 2.0), chapter(false, 5, 3.0)];
        let targets = chapters_to_change(false, &rows);
        assert_eq!(targets.iter().map(|c| c.chapter_number).collect::<Vec<_>>(), [2.0, 3.0], "EC-26");
        assert_eq!(chapter_status_update(false, &targets[1]).last_page_read, Some(0));
    }

    #[test]
    fn marking_unseen_resets_the_last_second_and_leaves_the_total() {
        let rows = [episode(true, 0, 1.0), episode(false, 40, 2.0), episode(false, 0, 3.0)];
        let targets = episodes_to_change(false, &rows);
        assert_eq!(targets.iter().map(|e| e.episode_number).collect::<Vec<_>>(), [1.0, 2.0]);
        let update = episode_status_update(false, &targets[1]);
        assert_eq!(update.last_second_seen, Some(0));
        assert_eq!(update.total_seconds, None, "total_seconds is never touched");
        assert_eq!(episode_status_update(true, &targets[0]).last_second_seen, None);
    }
}
