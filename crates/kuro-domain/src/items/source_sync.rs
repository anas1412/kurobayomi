//! The chapter sync rules that need only the source's list (`SyncChaptersWithSource.kt`, steps 1, 2, 4d and 6;
//! T-15a-035, TST-23, TST-25). The database diff, the update of stored rows and the download renames need the
//! repositories, so they stay with the sync job. The app converts `SChapter` into [`SourceChapter`], so this module
//! does not depend on `kuro-source`.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

/// One chapter as the source reports it, in domain terms. `source_order` is set by [`normalise_source_list`].
#[derive(Debug, Clone, PartialEq)]
pub struct SourceChapter {
    pub url: String,
    pub name: String,
    /// Epoch milliseconds, `0` when the source has no date.
    pub date_upload: i64,
    pub chapter_number: f32,
    pub scanlator: Option<String>,
    pub source_order: i64,
}

/// `NoChaptersException`: a non-local source returned an empty list. The sync stops and the database is not touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoChapters;

/// The Unicode whitespace of `ChapterSanitizer.kt`, plus the separators `- _ , :`.
const TRIM_CHARS: &[char] = &[
    ' ', '\u{0009}', '\u{000A}', '\u{000B}', '\u{000C}', '\u{000D}', '\u{0085}', '\u{00A0}', '\u{1680}', '\u{2000}',
    '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}',
    '\u{200A}', '\u{2028}', '\u{2029}', '\u{202F}', '\u{205F}', '\u{3000}', '-', '_', ',', ':',
];

/// `ChapterSanitizer.sanitize`: trim, remove the manga title from the front (exact and case-sensitive, a no-op when
/// the name does not start with it), then trim the characters of [`TRIM_CHARS`] (EC-CH-08, EC-CH-09).
pub fn sanitize_chapter_name(name: &str, manga_title: &str) -> String {
    let trimmed = name.trim();
    let without_title = trimmed.strip_prefix(manga_title).unwrap_or(trimmed);
    without_title.trim_matches(|c: char| TRIM_CHARS.contains(&c)).to_owned()
}

/// Steps 1 and 2 of the sync. A non-local source with an empty list is [`NoChapters`]. Otherwise duplicates by URL are
/// dropped (the first occurrence wins), each name is sanitised, and `source_order` is the position after that
/// de-duplication (EC-CH-01, EC-CH-02, EC-CH-08).
pub fn normalise_source_list(
    raw: &[SourceChapter],
    manga_title: &str,
    is_local: bool,
) -> Result<Vec<SourceChapter>, NoChapters> {
    if raw.is_empty() && !is_local {
        return Err(NoChapters);
    }
    let mut seen: HashSet<&str> = HashSet::new();
    let kept: Vec<&SourceChapter> = raw.iter().filter(|chapter| seen.insert(chapter.url.as_str())).collect();
    Ok(kept
        .into_iter()
        .enumerate()
        .map(|(position, chapter)| SourceChapter {
            name: sanitize_chapter_name(&chapter.name, manga_title),
            source_order: i64::try_from(position).unwrap_or(i64::MAX),
            ..chapter.clone()
        })
        .collect())
}

/// Step 4d: the upload date of each NEW chapter, given in list order. A chapter with no date takes `now` while no date
/// has been seen, and the largest date seen so far afterwards. A dated chapter raises that largest date (EC-CH-03).
pub fn new_chapter_upload_dates(raw_dates: &[i64], now: i64) -> Vec<i64> {
    let mut max_seen = 0_i64;
    raw_dates
        .iter()
        .map(|&date| {
            if date == 0 {
                if max_seen == 0 { now } else { max_seen }
            } else {
                max_seen = max_seen.max(date);
                date
            }
        })
        .collect()
}

/// Step 6: the fetch date of each NEW chapter in list order. The first gets `now + count` and the last `now + 1`, so
/// the source's most-recent-first order is kept (EC-CH-04).
pub fn new_chapter_fetch_dates(count: usize, now: i64) -> Vec<i64> {
    (0..count).map(|position| now + (count - position) as i64).collect()
}

/// RC-08: the syncs of one manga run one at a time, and syncs of different manga do not wait for each other.
// ponytail: one lock entry is kept per manga id synced this session and never evicted; drop idle entries if that grows.
#[derive(Default)]
pub struct MangaSyncLocks {
    locks: Mutex<HashMap<i64, Arc<AsyncMutex<()>>>>,
}

impl MangaSyncLocks {
    /// Waits until no other sync of `manga_id` holds the lock, then holds it until the guard is dropped.
    pub async fn lock(&self, manga_id: i64) -> OwnedMutexGuard<()> {
        let lock = {
            let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
            Arc::clone(locks.entry(manga_id).or_default())
        };
        lock.lock_owned().await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    fn chapter(url: &str, name: &str) -> SourceChapter {
        SourceChapter {
            url: url.into(),
            name: name.into(),
            date_upload: 0,
            chapter_number: -1.0,
            scanlator: None,
            source_order: 0,
        }
    }

    /// The sync cases of `fixtures/sync/chapter_lists.json` (steps 1 and 2) run against [`normalise_source_list`].
    #[test]
    fn the_sync_fixture_cases_hold() {
        let fixture = include_str!("../../tests/fixtures/sync/chapter_lists.json");
        let file: serde_json::Value = serde_json::from_str(fixture).unwrap();
        for case in file["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let raw: Vec<SourceChapter> = case["raw"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| chapter(c["url"].as_str().unwrap(), c["name"].as_str().unwrap()))
                .collect();
            let title = case["manga_title"].as_str().unwrap();
            let is_local = case["is_local"].as_bool().unwrap();
            let got = normalise_source_list(&raw, title, is_local);
            if let Some(expected) = case["expected_error"].as_str() {
                assert_eq!(expected, "NoChapters", "{name}");
                assert_eq!(got, Err(NoChapters), "{name}");
                continue;
            }
            let got: Vec<(String, String, i64)> =
                got.unwrap().into_iter().map(|c| (c.url, c.name, c.source_order)).collect();
            let want: Vec<(String, String, i64)> = case["expected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| {
                    (
                        c["url"].as_str().unwrap().to_owned(),
                        c["name"].as_str().unwrap().to_owned(),
                        c["source_order"].as_i64().unwrap(),
                    )
                })
                .collect();
            assert_eq!(got, want, "{name}");
        }
    }

    #[test]
    fn the_title_prefix_and_the_separators_are_removed() {
        assert_eq!(sanitize_chapter_name("  One Piece - Chapter 1 ", "One Piece"), "Chapter 1");
        assert_eq!(sanitize_chapter_name("-- Chapter 2 --", ""), "Chapter 2");
    }

    #[test]
    fn the_title_removal_is_case_sensitive() {
        assert_eq!(sanitize_chapter_name("Onepiece 2", "One Piece"), "Onepiece 2");
        assert_eq!(sanitize_chapter_name("one piece - x", "One Piece"), "one piece - x");
    }

    #[test]
    fn duplicates_by_url_keep_the_first_and_the_order_is_the_position_after_that() {
        let raw = [chapter("c3", "3"), chapter("c2", "2"), chapter("c2", "2 again"), chapter("c1", "1")];
        let out = normalise_source_list(&raw, "M", false).unwrap();
        let urls: Vec<&str> = out.iter().map(|c| c.url.as_str()).collect();
        let orders: Vec<i64> = out.iter().map(|c| c.source_order).collect();
        assert_eq!(urls, ["c3", "c2", "c1"]);
        assert_eq!(orders, [0, 1, 2]);
        assert_eq!(out[1].name, "2");
    }

    #[test]
    fn an_empty_list_is_an_error_for_a_remote_source_and_fine_for_a_local_one() {
        assert_eq!(normalise_source_list(&[], "M", false), Err(NoChapters));
        assert_eq!(normalise_source_list(&[], "M", true), Ok(Vec::new()));
    }

    #[test]
    fn names_are_sanitised_during_the_normalisation() {
        let out = normalise_source_list(&[chapter("c1", "M - Chapter 1")], "M", false).unwrap();
        assert_eq!(out[0].name, "Chapter 1");
    }

    #[test]
    fn new_chapters_get_fetch_dates_counting_down_from_now_plus_count() {
        assert_eq!(new_chapter_fetch_dates(3, 1000), [1003, 1002, 1001]);
        assert!(new_chapter_fetch_dates(0, 1000).is_empty());
    }

    #[test]
    fn a_chapter_without_a_date_takes_now_until_a_date_is_seen() {
        assert_eq!(new_chapter_upload_dates(&[0, 0, 0], 500), [500, 500, 500]);
    }

    #[test]
    fn a_chapter_without_a_date_takes_the_largest_date_seen_before_it() {
        assert_eq!(new_chapter_upload_dates(&[0, 1000, 0], 500), [500, 1000, 1000]);
        assert_eq!(new_chapter_upload_dates(&[2000, 0, 1000], 500), [2000, 2000, 1000]);
    }

    #[tokio::test]
    async fn syncs_of_one_manga_run_one_at_a_time() {
        let locks = Arc::new(MangaSyncLocks::default());
        let inside = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let (locks, inside, peak) = (locks.clone(), inside.clone(), peak.clone());
            tasks.push(tokio::spawn(async move {
                let _guard = locks.lock(7).await;
                let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                tokio::task::yield_now().await;
                inside.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_sync_of_another_manga_does_not_wait_for_the_held_lock() {
        let locks = MangaSyncLocks::default();
        let _held = locks.lock(1).await;
        let other = tokio::time::timeout(Duration::from_secs(1), locks.lock(2)).await;
        assert!(other.is_ok(), "a different manga must not wait");
    }

    #[tokio::test]
    async fn a_second_sync_of_the_same_manga_waits_until_the_first_guard_drops() {
        let locks = MangaSyncLocks::default();
        let first = locks.lock(3).await;
        let blocked = tokio::time::timeout(Duration::from_millis(50), locks.lock(3)).await;
        assert!(blocked.is_err(), "the same manga must wait");
        drop(first);
        let second = tokio::time::timeout(Duration::from_secs(1), locks.lock(3)).await;
        assert!(second.is_ok());
    }
}
