//! The delete-after-read rule (T-43a-032). When a chapter is completed, the chapter `slots` positions before it in the
//! reader list is queued for deletion. The queue is flushed when the reader closes, so a chapter is removed only
//! after the reader is done with it. A negative slot count turns the feature off (E-DEL-1).
//!
//! The queue survives a restart (E-DEL-4): [`QueueFile`] keeps it in one file in the platform data directory, so
//! chapters queued by a reader that never closed are deleted by the next reader that does. Each entry names its
//! manga, because the next reader may be open on another one.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::model::ReaderChapter;
use super::rules;

/// The queue's file name in the data directory.
pub const FILE_NAME: &str = "reader_delete_queue.json";

/// A read chapter whose download waits for the reader to close.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub manga_id: i64,
    pub chapter_id: i64,
}

/// The list index to delete for a completed chapter. `None` when the feature is off, or when the position is before
/// the first chapter. Slot `0` targets the completed chapter itself (E-DEL-2). The slot rule is [`rules::delete_index`].
pub fn delete_target(chapters: &[Rc<ReaderChapter>], completed: usize, slots: i64) -> Option<usize> {
    if completed >= chapters.len() {
        return None;
    }
    rules::delete_index(completed, slots.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
}

/// The chapters waiting for the reader to close. Each one is queued once, in the order it was completed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct DeleteQueue {
    entries: Vec<Pending>,
}

impl DeleteQueue {
    /// Queues a chapter that has been read. One already queued stays where it is.
    pub fn push(&mut self, entry: Pending) {
        if !self.entries.contains(&entry) {
            self.entries.push(entry);
        }
    }

    /// Takes every queued chapter, leaving the queue empty. The caller deletes them when the reader closes.
    pub fn take(&mut self) -> Vec<Pending> {
        std::mem::take(&mut self.entries)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Serialises every read-modify-write of a queue file in this process (the reader and a flush run on different threads).
static FILE_LOCK: Mutex<()> = Mutex::new(());

/// The queue in its file. A missing or unreadable file is an empty queue.
#[derive(Clone, Debug)]
pub struct QueueFile {
    path: PathBuf,
}

impl QueueFile {
    pub fn in_dir(data_dir: &Path) -> Self {
        Self { path: data_dir.join(FILE_NAME) }
    }

    fn read(&self) -> Vec<Pending> {
        std::fs::read(&self.path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }

    fn write(&self, entries: &[Pending]) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(entries).map_err(std::io::Error::other)?)?;
        std::fs::rename(&tmp, &self.path)
    }

    fn update(&self, change: impl FnOnce(&mut DeleteQueue)) -> std::io::Result<()> {
        let _guard = FILE_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
        let mut queue = DeleteQueue { entries: Vec::new() };
        self.read().into_iter().for_each(|entry| queue.push(entry));
        change(&mut queue);
        self.write(&queue.entries)
    }

    /// Adds a chapter to the file.
    pub fn add(&self, entry: Pending) -> std::io::Result<()> {
        self.update(|queue| queue.push(entry))
    }

    /// Every queued chapter, oldest first.
    pub fn list(&self) -> Vec<Pending> {
        let _guard = FILE_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
        self.read()
    }

    /// Removes the chapters that were dealt with. Chapters queued since the list was read stay.
    pub fn remove(&self, done: &[Pending]) -> std::io::Result<()> {
        self.update(|queue| {
            let kept: Vec<Pending> = queue.take().into_iter().filter(|entry| !done.contains(entry)).collect();
            kept.into_iter().for_each(|entry| queue.push(entry));
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_data::chapter::Chapter;

    fn list(n: usize) -> Vec<Rc<ReaderChapter>> {
        (0..n)
            .map(|i| {
                let mut c = Chapter::create();
                c.id = i as i64;
                ReaderChapter::new(c)
            })
            .collect()
    }

    #[test]
    fn a_negative_slot_count_turns_the_feature_off() {
        assert_eq!(delete_target(&list(5), 3, -1), None);
        assert_eq!(delete_target(&list(5), 3, -7), None);
    }

    #[test]
    fn slot_zero_targets_the_completed_chapter_and_slot_n_the_one_n_positions_before() {
        assert_eq!(delete_target(&list(5), 3, 0), Some(3));
        assert_eq!(delete_target(&list(5), 3, 2), Some(1));
    }

    #[test]
    fn a_position_before_the_first_chapter_targets_nothing() {
        assert_eq!(delete_target(&list(5), 1, 2), None);
    }

    fn pending(chapter_id: i64) -> Pending {
        Pending { manga_id: 1, chapter_id }
    }

    #[test]
    fn each_chapter_is_queued_once_and_taking_empties_the_queue() {
        let mut queue = DeleteQueue::default();
        queue.push(pending(4));
        queue.push(pending(9));
        queue.push(pending(4));
        assert_eq!(queue.take(), vec![pending(4), pending(9)]);
        assert!(queue.is_empty());
    }

    #[test]
    fn the_file_keeps_the_queue_across_a_restart_and_removes_only_what_was_dealt_with() {
        let dir = tempfile::tempdir().unwrap();
        let file = QueueFile::in_dir(&dir.path().join("data"));
        assert!(file.list().is_empty());
        file.add(pending(4)).unwrap();
        file.add(pending(9)).unwrap();
        file.add(pending(4)).unwrap();
        // a new handle, as after a restart, sees the same queue
        let after_restart = QueueFile::in_dir(&dir.path().join("data"));
        assert_eq!(after_restart.list(), vec![pending(4), pending(9)]);
        after_restart.remove(&[pending(4)]).unwrap();
        after_restart.add(pending(12)).unwrap();
        assert_eq!(file.list(), vec![pending(9), pending(12)]);
    }

    #[test]
    fn an_unreadable_file_is_an_empty_queue() {
        let dir = tempfile::tempdir().unwrap();
        let file = QueueFile::in_dir(dir.path());
        std::fs::write(dir.path().join(FILE_NAME), b"[").unwrap();
        assert!(file.list().is_empty());
        file.add(pending(1)).unwrap();
        assert_eq!(file.list(), vec![pending(1)]);
    }
}
