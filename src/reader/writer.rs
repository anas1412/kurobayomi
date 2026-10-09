//! The database writer (T-43a-088). One thread owns the write path. Commands arrive on a channel, are coalesced
//! per chapter before the write, and run through the repositories. Nothing here runs on the GTK thread.
//!
//! Progress and bookmark writes use the chapter repository. Viewer flags use the manga repository's
//! `set_viewer_bits`, which merges under the mask inside one transaction.
//!
//! Reader preferences do not go through this actor: `Pref::set` updates the preference store's memory at once and
//! the store's own flusher thread saves it, so the GTK thread never waits on a file (see `settings_window`).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use kuro_data::chapter::{ChapterRepo, ChapterUpdate};
use kuro_data::manga::MangaRepo;
use kuro_db::Db;

use super::session::ReaderStore;

/// One write the reader asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Progress { chapter_id: i64, last_page_read: i64, read: bool },
    Bookmark { chapter_id: i64, bookmark: bool },
    ViewerBits { manga_id: i64, value: i64, mask: i64 },
}

/// How long [`ReaderStore::flush`] waits for the writer before it gives up and goes on.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(2);

/// What travels on the writer's channel: a write, or a barrier that is acknowledged once every write sent
/// before it has run.
enum Msg {
    Write(Command),
    Flush(Sender<()>),
}

/// Coalesces one batch: per chapter only the last progress and the last bookmark are written, and viewer-bit
/// writes are kept in order, since each one merges into the previous value.
pub fn coalesce(batch: Vec<Command>) -> Vec<Command> {
    let mut progress: BTreeMap<i64, Command> = BTreeMap::new();
    let mut bookmark: BTreeMap<i64, Command> = BTreeMap::new();
    let mut bits = Vec::new();
    for command in batch {
        match &command {
            Command::Progress { chapter_id, .. } => {
                progress.insert(*chapter_id, command);
            }
            Command::Bookmark { chapter_id, .. } => {
                bookmark.insert(*chapter_id, command);
            }
            Command::ViewerBits { .. } => bits.push(command),
        }
    }
    bits.into_iter().chain(progress.into_values()).chain(bookmark.into_values()).collect()
}

/// The writer's sending half, held by the reader session.
#[derive(Clone)]
pub struct WriterHandle {
    tx: Sender<Msg>,
}

impl WriterHandle {
    /// Starts the writer thread over `db`. It runs until every handle is dropped.
    pub fn spawn(db: Arc<Db>) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("kuro-reader-writer".into())
            .spawn(move || run(db, rx))
            .expect("the reader writer thread starts");
        Self { tx }
    }
}

impl ReaderStore for WriterHandle {
    fn save_progress(&self, chapter_id: i64, last_page_read: i64, read: bool) {
        self.send(Command::Progress { chapter_id, last_page_read, read });
    }

    fn set_bookmark(&self, chapter_id: i64, bookmark: bool) {
        self.send(Command::Bookmark { chapter_id, bookmark });
    }

    fn set_viewer_flags(&self, manga_id: i64, value: i64, mask: i64) {
        self.send(Command::ViewerBits { manga_id, value, mask });
    }

    /// Waits until the writer has run everything sent before this call (T-43a-025). Any later write that goes
    /// through this writer, such as the history upsert of T-43a-033, is covered the same way.
    fn flush(&self) {
        let (ack, done) = mpsc::channel();
        if self.tx.send(Msg::Flush(ack)).is_err() || done.recv_timeout(FLUSH_TIMEOUT).is_err() {
            tracing::warn!("reader writer did not confirm a flush");
        }
    }
}

impl WriterHandle {
    fn send(&self, command: Command) {
        if self.tx.send(Msg::Write(command)).is_err() {
            tracing::warn!("reader writer is gone; a progress write was dropped");
        }
    }
}

/// Splits one batch of messages into its writes and its flush acknowledgements.
fn split(batch: Vec<Msg>) -> (Vec<Command>, Vec<Sender<()>>) {
    let (mut writes, mut acks) = (Vec::new(), Vec::new());
    for msg in batch {
        match msg {
            Msg::Write(command) => writes.push(command),
            Msg::Flush(ack) => acks.push(ack),
        }
    }
    (writes, acks)
}

fn run(db: Arc<Db>, rx: Receiver<Msg>) {
    let chapters = ChapterRepo::new(Arc::clone(&db));
    let mangas = MangaRepo::new(db);
    while let Ok(first) = rx.recv() {
        let mut batch = vec![first];
        while let Ok(next) = rx.try_recv() {
            batch.push(next);
        }
        let (writes, acks) = split(batch);
        for command in coalesce(writes) {
            let result = match &command {
                Command::Progress { chapter_id, last_page_read, read } => {
                    let mut update = ChapterUpdate::new(*chapter_id);
                    update.read = Some(*read);
                    update.last_page_read = Some(*last_page_read);
                    chapters.update(&update).map(|_| ())
                }
                Command::Bookmark { chapter_id, bookmark } => {
                    let mut update = ChapterUpdate::new(*chapter_id);
                    update.bookmark = Some(*bookmark);
                    chapters.update(&update).map(|_| ())
                }
                Command::ViewerBits { manga_id, value, mask } => {
                    mangas.set_viewer_bits(*manga_id, *value, *mask).map(|_| ())
                }
            };
            if let Err(e) = result {
                tracing::warn!(error = %e, ?command, "reader write failed");
            }
        }
        for ack in acks {
            let _ = ack.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_coalesced_per_chapter_to_the_last_write() {
        let out = coalesce(vec![
            Command::Progress { chapter_id: 1, last_page_read: 0, read: false },
            Command::Progress { chapter_id: 2, last_page_read: 4, read: false },
            Command::Progress { chapter_id: 1, last_page_read: 3, read: true },
        ]);
        assert_eq!(
            out,
            vec![
                Command::Progress { chapter_id: 1, last_page_read: 3, read: true },
                Command::Progress { chapter_id: 2, last_page_read: 4, read: false },
            ]
        );
    }

    #[test]
    fn a_batch_splits_into_its_writes_and_its_flush_acknowledgements() {
        let (ack, _done) = mpsc::channel();
        let (writes, acks) = split(vec![
            Msg::Write(Command::Bookmark { chapter_id: 1, bookmark: true }),
            Msg::Flush(ack),
            Msg::Write(Command::Bookmark { chapter_id: 2, bookmark: true }),
        ]);
        assert_eq!(writes.len(), 2);
        assert_eq!(acks.len(), 1);
    }

    #[test]
    fn flush_returns_only_after_the_earlier_writes_are_in_the_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = kuro_db::schema_manga::db_path(dir.path());
        let writer = kuro_db::schema_manga::open(&path).unwrap();
        let db = Arc::new(Db::new(&path, writer).unwrap());
        let manga_id = MangaRepo::new(Arc::clone(&db)).insert(&kuro_data::manga::Manga::create()).unwrap();
        let mut chapter = kuro_data::chapter::Chapter::create();
        chapter.manga_id = manga_id;
        chapter.url = "/c/1".into();
        let chapter_id = ChapterRepo::new(Arc::clone(&db)).add_all(&[chapter]).unwrap()[0].id;

        let handle = WriterHandle::spawn(Arc::clone(&db));
        handle.save_progress(chapter_id, 4, true);
        handle.flush();
        let stored = ChapterRepo::new(db).by_id(chapter_id).unwrap().unwrap();
        assert_eq!((stored.last_page_read, stored.read), (4, true));
    }

    #[test]
    fn viewer_bit_writes_keep_their_order_and_run_before_progress() {
        let out = coalesce(vec![
            Command::ViewerBits { manga_id: 9, value: 2, mask: 0x7 },
            Command::Progress { chapter_id: 1, last_page_read: 1, read: false },
            Command::ViewerBits { manga_id: 9, value: 0x10, mask: 0x38 },
        ]);
        assert_eq!(out[0], Command::ViewerBits { manga_id: 9, value: 2, mask: 0x7 });
        assert_eq!(out[1], Command::ViewerBits { manga_id: 9, value: 0x10, mask: 0x38 });
        assert_eq!(out.len(), 3);
    }
}
