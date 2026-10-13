//! Shared value types of the unified queue: projections of `Entry` and `Item`, the source key, live progress and the
//! queued unit (docs/design/unified-downloads.md 2.2).

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use kuro_data::entry::Entry;
use kuro_data::item::Item;
use kuro_platform::kind::Kind;

use crate::model::DownloadState;

/// Total queued units that make the bulk warning show (decision: 30 for both kinds).
/// Now, in milliseconds since the epoch.
pub fn unix_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

pub const BULK_TOTAL_THRESHOLD: usize = 30;
/// Units of one source that make the bulk warning show.
pub const BULK_PER_SOURCE_THRESHOLD: usize = 15;

/// What a download needs to know about an entry (a manga or an anime).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryRef {
    pub id: i64,
    pub kind: Kind,
    pub source: i64,
    pub url: String,
    pub title: String,
    /// The details a manga's `ComicInfo.xml` carries.
    pub description: Option<String>,
    pub author: Option<String>,
    pub artist: Option<String>,
    pub genre: Option<Vec<String>>,
    pub status: i64,
}

impl EntryRef {
    /// An entry with no details (a pending delete or a test needs only these).
    pub fn bare(id: i64, kind: Kind, source: i64, url: String, title: String) -> Self {
        Self { id, kind, source, url, title, description: None, author: None, artist: None, genre: None, status: 0 }
    }
}

impl From<&Entry> for EntryRef {
    fn from(e: &Entry) -> Self {
        Self {
            id: e.id,
            kind: e.kind,
            source: e.source,
            url: e.url.clone(),
            title: e.title.clone(),
            description: e.description.clone(),
            author: e.author.clone(),
            artist: e.artist.clone(),
            genre: e.genre.clone(),
            status: e.status,
        }
    }
}

/// What a download needs to know about a chapter or an episode.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitRef {
    pub id: i64,
    pub entry_id: i64,
    pub url: String,
    pub name: String,
    pub scanlator: Option<String>,
    pub done: bool,
    pub bookmark: bool,
    pub fillermark: bool,
    pub source_order: i64,
    pub date_upload: i64,
    pub number: f64,
}

impl From<&Item> for UnitRef {
    fn from(i: &Item) -> Self {
        Self {
            id: i.id,
            entry_id: i.entry_id,
            url: i.url.clone(),
            name: i.name.clone(),
            scanlator: i.scanlator.clone(),
            done: i.done,
            bookmark: i.bookmark,
            fillermark: i.fillermark,
            source_order: i.source_order,
            date_upload: i.date_upload,
            number: i.number,
        }
    }
}

/// Whether the units of a source can be run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    Live,
    /// A local source: refused at enqueue.
    Local,
    /// Not installed: the unit waits.
    Stub,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceKey {
    pub id: i64,
    pub name: String,
    pub lang: String,
    /// A source on the user's own machine: the bulk warning leaves it out.
    pub unmetered: bool,
    pub availability: Availability,
}

impl SourceKey {
    /// `"Name (LANG)"`, or `"Name"` when the language is empty.
    pub fn display_name(&self) -> String {
        if self.lang.is_empty() {
            self.name.clone()
        } else {
            format!("{} ({})", self.name, self.lang)
        }
    }
}

/// Live numbers a runner writes and the actor reads. Lock-free, so a runner never waits for the actor.
#[derive(Default)]
pub struct UnitProgress {
    percent: AtomicU8,
    done: AtomicU32,
    /// 0 means the page count is not known (anime, or before the page list).
    total: AtomicU32,
    version: AtomicU64,
}

impl UnitProgress {
    pub fn set_percent(&self, percent: u8) {
        self.percent.store(percent.min(100), Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Release);
    }

    pub fn set_pages(&self, done: u32, total: u32) {
        self.done.store(done, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Release);
    }

    pub fn percent(&self) -> u8 {
        self.percent.load(Ordering::Relaxed)
    }

    /// `(done, total)` once a page count is known.
    pub fn pages(&self) -> Option<(u32, u32)> {
        let total = self.total.load(Ordering::Relaxed);
        (total > 0).then(|| (self.done.load(Ordering::Relaxed), total))
    }

    /// Bumps on every set; the actor flushes a progress event when it moved.
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }
}

/// One unit in the queue. A state change swaps the `Arc` in the lane (`Arc::make_mut`); `progress` is the live part.
#[derive(Clone)]
pub struct QueuedUnit {
    pub kind: Kind,
    pub entry: EntryRef,
    pub unit: UnitRef,
    pub source: SourceKey,
    /// Queue, Downloading or Error. A downloaded unit never stays queued.
    pub state: DownloadState,
    /// The runner cannot reach the source: the scheduler skips the unit.
    pub waiting: bool,
    pub progress: Arc<UnitProgress>,
}

impl QueuedUnit {
    /// A unit as it enters the queue: state Queue, not waiting, fresh progress.
    pub fn new(kind: Kind, entry: EntryRef, unit: UnitRef, source: SourceKey) -> Self {
        Self { kind, entry, unit, source, state: DownloadState::Queue, waiting: false, progress: Arc::default() }
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;

    pub fn source(id: i64) -> SourceKey {
        SourceKey { id, name: format!("Source {id}"), lang: "en".to_owned(), unmetered: false, availability: Availability::Live }
    }

    pub fn queued(kind: Kind, source_id: i64, unit_id: i64, entry_id: i64) -> QueuedUnit {
        let entry = EntryRef::bare(entry_id, kind, source_id, format!("/e{entry_id}"), format!("Title {entry_id}"));
        let unit = UnitRef {
            id: unit_id,
            entry_id,
            url: format!("/u{unit_id}"),
            name: format!("Unit {unit_id}"),
            scanlator: None,
            done: false,
            bookmark: false,
            fillermark: false,
            source_order: unit_id,
            date_upload: 0,
            number: unit_id as f64,
        };
        QueuedUnit::new(kind, entry, unit, source(source_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_adds_the_language_when_there_is_one() {
        let mut s = fixtures::source(1);
        assert_eq!(s.display_name(), "Source 1 (en)");
        s.lang.clear();
        assert_eq!(s.display_name(), "Source 1");
    }

    #[test]
    fn progress_versions_move_on_every_set_and_pages_need_a_total() {
        let p = UnitProgress::default();
        assert_eq!((p.version(), p.pages(), p.percent()), (0, None, 0));
        p.set_percent(150);
        assert_eq!((p.percent(), p.version()), (100, 1));
        p.set_pages(2, 0);
        assert_eq!(p.pages(), None);
        p.set_pages(2, 10);
        assert_eq!((p.pages(), p.version()), (Some((2, 10)), 3));
    }
}
