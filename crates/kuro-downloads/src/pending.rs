//! Chapters waiting for the reader to exit before their download is deleted (T-23a-028; port of
//! `MangaDownloadPendingDeleter`). A chapter is added at enqueue time after the delete filter, deduplicated by chapter
//! id, and taken in one transaction on flush. The flush plan drops rows of a missing manga and applies the category
//! exclusion again against the live categories (EC-106).

use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, params};

use crate::manager::{DeleteRules, chapters_to_delete};
use crate::model::{ChapterRef, MangaRef};
use crate::store::StoreError;

/// DDL for `manga_pending_delete`.
pub const SCHEMA: &str = include_str!("../sql/manga_pending_delete.sql");

/// The chapters pending for one manga, in insertion order.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingManga {
    pub manga: MangaRef,
    pub chapters: Vec<ChapterRef>,
}

pub struct MangaPendingStore {
    conn: Mutex<Connection>,
}

impl MangaPendingStore {
    pub fn open(conn: Connection) -> Result<Self, StoreError> {
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::open(Connection::open_in_memory()?)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.conn.lock().map_err(|_| StoreError::Poisoned)
    }

    /// Adds the chapters after the delete filter of `rules` (EC-94, EC-95). Returns how many rows were new
    /// (`INSERT OR IGNORE`, TST-44, EC-90).
    pub fn add_chapters(
        &self,
        manga: &MangaRef,
        chapters: &[ChapterRef],
        rules: &DeleteRules,
        now: i64,
    ) -> Result<usize, StoreError> {
        let selected = chapters_to_delete(chapters.to_vec(), rules);
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let mut added = 0;
        for c in &selected {
            added += tx.execute(
                "INSERT OR IGNORE INTO manga_pending_delete(chapter_id, manga_id, manga_title, manga_url, source_id, \
                 chapter_name, chapter_url, scanlator, enqueued_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![c.id, manga.id, manga.title, manga.url, manga.source_id, c.name, c.url, c.scanlator, now],
            )?;
        }
        tx.commit()?;
        Ok(added)
    }

    /// Reads every pending chapter, grouped by manga in insertion order, and deletes the rows in the same transaction
    /// (TST-45, EC-91, EC-92).
    pub fn take_all(&self) -> Result<Vec<PendingManga>, StoreError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let rows: Vec<(MangaRef, ChapterRef)> = {
            let mut stmt = tx.prepare(
                "SELECT chapter_id, manga_id, manga_title, manga_url, source_id, chapter_name, chapter_url, scanlator \
                 FROM manga_pending_delete ORDER BY enqueued_at, chapter_id",
            )?;
            let mapped = stmt.query_map([], |row| {
                let manga = MangaRef {
                    id: row.get(1)?,
                    url: row.get(3)?,
                    title: row.get(2)?,
                    source_id: row.get(4)?,
                    description: None,
                    author: None,
                    artist: None,
                    genre: None,
                    status: 0,
                };
                let chapter = ChapterRef {
                    id: row.get(0)?,
                    manga_id: manga.id,
                    url: row.get(6)?,
                    name: row.get(5)?,
                    scanlator: row.get(7)?,
                    read: false,
                    bookmark: false,
                    source_order: 0,
                    chapter_number: 0.0,
                    date_upload: 0,
                };
                Ok((manga, chapter))
            })?;
            mapped.collect::<Result<_, _>>()?
        };
        tx.execute("DELETE FROM manga_pending_delete", [])?;
        tx.commit()?;
        Ok(group(rows))
    }
}

fn group(rows: Vec<(MangaRef, ChapterRef)>) -> Vec<PendingManga> {
    let mut groups: Vec<PendingManga> = Vec::new();
    for (manga, chapter) in rows {
        match groups.iter_mut().find(|g| g.manga.id == manga.id) {
            Some(g) => g.chapters.push(chapter),
            None => groups.push(PendingManga { manga, chapters: vec![chapter] }),
        }
    }
    groups
}

/// The flush plan (EC-93, EC-106): a manga that is no longer in the library is dropped, and the category exclusion is
/// applied again with the live rules of that manga, because a chapter may have been moved since it was added.
pub fn flush_plan(
    pending: Vec<PendingManga>,
    live_rules: impl Fn(i64) -> Option<DeleteRules>,
) -> Vec<PendingManga> {
    pending
        .into_iter()
        .filter_map(|p| {
            let rules = live_rules(p.manga.id)?;
            let chapters = chapters_to_delete(p.chapters, &rules);
            (!chapters.is_empty()).then_some(PendingManga { manga: p.manga, chapters })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{chapter, manga};

    fn store() -> MangaPendingStore {
        MangaPendingStore::open_in_memory().unwrap()
    }

    #[test]
    fn adding_the_same_chapter_twice_keeps_one_row() {
        let s = store();
        let m = manga(1, 1, "Berserk");
        let rules = DeleteRules { remove_bookmarked: true, ..Default::default() };
        assert_eq!(s.add_chapters(&m, &[chapter(10, 1, "Ch 10")], &rules, 1).unwrap(), 1);
        assert_eq!(s.add_chapters(&m, &[chapter(10, 1, "Ch 10")], &rules, 2).unwrap(), 0, "EC-90");
    }

    #[test]
    fn take_all_returns_the_rows_grouped_and_empties_the_table() {
        let s = store();
        let m = manga(1, 1, "Berserk");
        let rules = DeleteRules { remove_bookmarked: true, ..Default::default() };
        s.add_chapters(&m, &[chapter(10, 1, "Ch 10"), chapter(11, 1, "Ch 11")], &rules, 1).unwrap();
        let taken = s.take_all().unwrap();
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].chapters.iter().map(|c| c.id).collect::<Vec<_>>(), vec![10, 11], "EC-91");
        assert!(s.take_all().unwrap().is_empty(), "the table is empty after the take");
    }

    // TST-46
    #[test]
    fn the_filter_runs_at_enqueue_time() {
        let s = store();
        let m = manga(1, 1, "Berserk");
        let mut bookmarked = chapter(20, 1, "Ch 20");
        bookmarked.bookmark = true;
        let protected = DeleteRules::default();
        assert_eq!(s.add_chapters(&m, &[bookmarked], &protected, 1).unwrap(), 0, "EC-94: bookmarks stay");
        let mut read = chapter(21, 1, "Ch 21");
        read.read = true;
        let excluded = DeleteRules { exclude_categories: vec![0], remove_bookmarked: true, ..Default::default() };
        assert_eq!(s.add_chapters(&m, &[read], &excluded, 1).unwrap(), 0, "EC-95: read chapter in an excluded category");
    }

    #[test]
    fn the_flush_drops_a_missing_manga_and_rechecks_exclusions() {
        let m1 = manga(1, 1, "Gone");
        let m2 = manga(2, 1, "Live");
        let mut read = chapter(31, 2, "Ch 31");
        read.read = true;
        let pending = vec![
            PendingManga { manga: m1, chapters: vec![chapter(30, 1, "Ch 30")] },
            PendingManga { manga: m2, chapters: vec![read, chapter(32, 2, "Ch 32")] },
        ];
        let live = |id: i64| match id {
            2 => Some(DeleteRules { exclude_categories: vec![0], ..Default::default() }),
            _ => None,
        };
        let plan = flush_plan(pending, live);
        assert_eq!(plan.len(), 1, "EC-93: the missing manga is dropped");
        assert_eq!(plan[0].chapters.iter().map(|c| c.id).collect::<Vec<_>>(), vec![32], "EC-106: read chapter kept");
    }
}
