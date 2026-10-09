//! Pure reader rules of the behaviour tables (`p1-edge-cases-and-tests.md` sections 7 and 10): the remove-after-read
//! target (E-DEL-1, E-DEL-2), the page-change flash (E-FLASH-1, E-FLASH-2) and the saved-page file name (E-PATH-1).
//!
//! The history rule (E-HIST-1) lives in `kuro-data` (`MangaHistoryRepo::upsert` adds the session time and a zero
//! session adds nothing), so the test here runs the real repository. Nothing in this file touches GTK.

use std::time::Duration;

use kuro_prefs::reader::FlashColor;

/// The title budget of a saved page name in UTF-8 bytes, suffix included (`DiskUtil.MAX_FILE_NAME_BYTES`).
pub const MAX_FILE_NAME_BYTES: usize = 250;

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

/// The colour on screen `elapsed` into a flash, or `None` once it is over.
pub fn flash_colour_at(colour: FlashColor, duration: Duration, elapsed: Duration) -> Option<(f64, f64, f64)> {
    if elapsed >= duration {
        return None;
    }
    flash_phases(colour, duration).into_iter().rev().find(|phase| elapsed >= phase.start).map(|phase| phase.rgb)
}

const FORBIDDEN: &str = "\"*/:<>?\\|";

/// `DiskUtil.buildValidFilename` for the part of a name that is sanitised: dots and spaces trimmed at both ends,
/// forbidden and control characters become `_`, and an empty result reads `(invalid)`.
fn valid_filename(raw: &str) -> String {
    let trimmed = raw.trim_matches(|c| c == '.' || c == ' ');
    if trimmed.is_empty() {
        return "(invalid)".to_owned();
    }
    trimmed.chars().map(|c| if c <= '\u{1f}' || c == '\u{7f}' || FORBIDDEN.contains(c) { '_' } else { c }).collect()
}

/// The name of a saved page: `<title> - <chapter>` cut to the byte budget left by the suffix, sanitised, then
/// ` - <page number>` appended unsanitised (E-PATH-1). The cut keeps whole characters.
pub fn save_filename(title: &str, chapter_name: &str, page_number: usize) -> String {
    let suffix = format!(" - {page_number}");
    let budget = MAX_FILE_NAME_BYTES.saturating_sub(suffix.len());
    let head = format!("{title} - {chapter_name}");
    format!("{}{suffix}", valid_filename(kuro_core::text::take_bytes(&head, budget)))
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

    #[test]
    fn flash_colour_white_black_halves() {
        let duration = Duration::from_millis(100);
        let at = |ms| flash_colour_at(FlashColor::WHITE_BLACK, duration, Duration::from_millis(ms));
        assert_eq!(at(25), Some(WHITE));
        assert_eq!(at(75), Some(BLACK));
        assert_eq!(at(100), None, "the flash is over at the full duration");
        let whole = |c, ms| flash_colour_at(c, duration, Duration::from_millis(ms));
        assert_eq!(whole(FlashColor::WHITE, 75), Some(WHITE), "white keeps white for the whole duration");
        assert_eq!(whole(FlashColor::BLACK, 75), Some(BLACK));
    }

    #[test]
    fn filename_suffix_and_limit() {
        assert_eq!(save_filename("Title", "Ch. 1", 7), "Title - Ch. 1 - 7");
        assert_eq!(save_filename("A/B", "C:D", 12), "A_B - C_D - 12");
        assert_eq!(valid_filename(" .. "), "(invalid)");
        let long = save_filename(&"x".repeat(400), "chapter", 123);
        assert!(long.len() <= MAX_FILE_NAME_BYTES, "the whole name, suffix included, fits the budget");
        assert!(long.ends_with(" - 123"));
    }

    #[test]
    fn take_bytes_utf8_boundary() {
        // a 300-byte multi-byte title is cut without a broken character
        let title = "漫".repeat(100);
        assert_eq!(title.len(), 300);
        let name = save_filename(&title, "c", 1);
        assert!(name.len() <= MAX_FILE_NAME_BYTES);
        assert!(!name.contains('\u{FFFD}'));
        assert!(name.ends_with(" - 1"));
        let head = name.strip_suffix(" - 1").unwrap();
        assert!(head.chars().all(|c| c == '漫'), "only whole characters are kept");
        assert_eq!(head.len() % 3, 0);
    }

    fn db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().unwrap();
        let path = kuro_db::schema_manga::db_path(dir.path());
        let writer = kuro_db::schema_manga::open(&path).unwrap();
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
