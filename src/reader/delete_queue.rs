//! The delete-after-read rule (T-43a-032). When a chapter is completed, the chapter `slots` positions before it in the
//! reader list is queued for deletion. The service flushes its pending deletes when the reader closes, so a chapter is removed only
//! after the reader is done with it. A negative slot count turns the feature off (E-DEL-1).
//!
//! The queue survives a restart (E-DEL-4): each completed chapter is handed at once to the download service
//! (`Downloads::defer_delete`), which keeps it in its pending table. The in-memory queue below only keeps a chapter
//! from being handed twice.

use std::rc::Rc;

use super::model::ReaderChapter;
use super::rules;

/// A read chapter whose download waits for the reader to close.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// The chapters already handed to the download service, so each is handed once.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct DeleteQueue {
    entries: Vec<Pending>,
}

impl DeleteQueue {
    /// Remembers a chapter that has been read. `false` when it was already there.
    pub fn push(&mut self, entry: Pending) -> bool {
        let new = !self.entries.contains(&entry);
        if new {
            self.entries.push(entry);
        }
        new
    }
}

#[cfg(test)]
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
    use kuro_data::item::Item;
    use kuro_platform::kind::Kind;

    fn list(n: usize) -> Vec<Rc<ReaderChapter>> {
        (0..n)
            .map(|i| {
                let mut c = Item::create(Kind::Manga);
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
    fn each_chapter_is_queued_once() {
        let mut queue = DeleteQueue::default();
        assert!(queue.push(pending(4)));
        assert!(queue.push(pending(9)));
        assert!(!queue.push(pending(4)));
    }
}
