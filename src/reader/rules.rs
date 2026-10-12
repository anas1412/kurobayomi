//! Pure reader rules of the behaviour tables (`p1-edge-cases-and-tests.md` sections 7 and 10): the remove-after-read
//! target (E-DEL-1, E-DEL-2), the page-change flash (E-FLASH-1, E-FLASH-2) and the saved-page file name (E-PATH-1).
//!
//! The history rule (E-HIST-1) lives in `kuro-data` (`MangaHistoryRepo::upsert` adds the session time and a zero
//! session adds nothing), so the test here runs the real repository. Nothing in this file touches GTK.

use std::time::Duration;

use kuro_prefs::reader::FlashColor;

/// The chapter to delete after reading `current`, `slots` positions back. `-1` is off, and so is every other
/// negative value, so a negative number never deletes a chapter ahead (E-DEL-1). `0` is the current chapter itself
/// (E-DEL-2). The caller asks only when the chapter was completed.
pub fn delete_index(current: usize, slots: i32) -> Option<usize> {
    kuro_domain::download::policy::remove_after_read_index(current, slots)
}

/// The flash interval counter (`DisplayRefreshHost.flash`): the page whose count is a multiple of the interval
/// flashes, counting from the first call, so interval 3 flashes pages 0, 3, 6. Returns whether this call flashes
/// and the next count. The count is kept below the interval, so it never overflows.
pub fn flash_due(count: u32, interval: i32) -> (bool, u32) {
    let interval = interval.max(1) as u32;
    let count = count % interval;
    (count == 0, (count + 1) % interval)
}

/// One colour of a flash: it starts `start` after the flash began.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlashPhase {
    pub start: Duration,
    pub rgb: (f64, f64, f64),
}

const BLACK: (f64, f64, f64) = (0.0, 0.0, 0.0);
const WHITE: (f64, f64, f64) = (1.0, 1.0, 1.0);

/// The colours of one flash over `duration`. Black and white keep their colour for the whole duration. White and
/// black is white for the first half, then black (E-FLASH-2).
pub fn flash_phases(colour: FlashColor, duration: Duration) -> Vec<FlashPhase> {
    let first = |rgb| FlashPhase { start: Duration::ZERO, rgb };
    match colour {
        FlashColor::BLACK => vec![first(BLACK)],
        FlashColor::WHITE => vec![first(WHITE)],
        FlashColor::WHITE_BLACK => vec![first(WHITE), FlashPhase { start: duration / 2, rgb: BLACK }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use kuro_data::chapter::{Chapter, ChapterRepo};
    use kuro_data::history::{HistoryRecorder, MangaHistoryRepo};
    use kuro_data::manga::{Manga, MangaRepo};
    use kuro_db::Db;

    #[test]
    fn delete_slots_negative_other_than_minus_one() {
        assert_eq!(delete_index(5, -1), None);
        assert_eq!(delete_index(5, -2), None, "-2 does not reach a chapter ahead");
        assert_eq!(delete_index(5, i32::MIN), None);
    }

    #[test]
    fn delete_slots_zero_deletes_current() {
        assert_eq!(delete_index(5, 0), Some(5));
        assert_eq!(delete_index(5, 2), Some(3), "slots count back from the current chapter");
        assert_eq!(delete_index(1, 2), None, "before the first chapter there is nothing to delete");
    }

    #[test]
    fn flash_trigger_interval_counting() {
        let mut count = 0;
        let mut flashed = Vec::new();
        for page in 0..7 {
            let (due, next) = flash_due(count, 3);
            count = next;
            if due {
                flashed.push(page);
            }
        }
        assert_eq!(flashed, [0, 3, 6], "interval 3 flashes the first page and every third after it");
        let every: Vec<bool> = (0..4).scan(0, |c, _| { let (d, n) = flash_due(*c, 1); *c = n; Some(d) }).collect();
        assert_eq!(every, [true; 4], "interval 1 flashes every page");
        assert_eq!(flash_due(0, 0), (true, 0), "an interval below 1 counts as 1");
    }

    fn db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().unwrap();
        let path = kuro_db::schema_unified::db_path(dir.path());
        let writer = kuro_db::schema_unified::open_file(&path).unwrap();
        (dir, Arc::new(Db::new(&path, writer).unwrap()))
    }

    #[test]
    fn history_duration_not_overwritten_by_zero() {
        let (_dir, db) = db();
        let manga_id = MangaRepo::new(Arc::clone(&db)).insert(&Manga::create()).unwrap();
        let mut chapter = Chapter::create();
        chapter.manga_id = manga_id;
        chapter.url = "/c/1".into();
        let chapter_id = ChapterRepo::new(Arc::clone(&db)).add_all(&[chapter]).unwrap()[0].id;
        let repo = MangaHistoryRepo::new(db);
        let mut timer = HistoryRecorder::new(Arc::new(AtomicBool::new(false)));
        timer.start(1_000);
        timer.flush(&repo, chapter_id, 6_000);
        // a pause flush and a page change flush in a row: the second has no start time, so it writes zero
        timer.flush(&repo, chapter_id, 6_100);
        assert_eq!(repo.total_read_duration().unwrap(), 5_000);
        assert_eq!(repo.by_manga_id(manga_id).unwrap()[0].last_read, 6_100, "last_read still moves forward");
    }
}
