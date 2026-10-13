//! The anime downloads of the page (T-39b-063, T-39b-013): which episodes are on disk, queueing episodes, and deleting
//! them, all through the app's one download service. Without it nothing can be queued and the page shows no Download
//! menu. No GTK here. The calls block on the disk: run them on the blocking pool.

use std::collections::HashSet;
use std::sync::Arc;

use kuro_data::entry::Entry;
use kuro_data::item::Item;
use kuro_downloads::model::DownloadState;
use kuro_platform::kind::Kind;

use crate::entry_core::detail::SourceInfo;
use crate::entry_core::downloaded::download_states;
use crate::updates::download_host::{self, DownloadBackend};

/// The anime downloads of the page, over the one download service (`updates::download_host`), the same path the manga
/// page takes.
pub struct EntryDownloads {
    host: Arc<dyn DownloadBackend>,
    can_enqueue: bool,
}

impl EntryDownloads {
    /// Over the app's download service. Call it on the GTK thread: the service is found through the shell.
    pub fn new() -> Self {
        let service = crate::shell::host::Shell::get().is_some_and(|shell| shell.download_service().is_some());
        Self::over(download_host::current(), service)
    }

    pub(crate) fn over(host: Arc<dyn DownloadBackend>, can_enqueue: bool) -> Self {
        Self { host, can_enqueue }
    }

    /// Whether episodes can be queued: the download service runs.
    pub fn can_enqueue(&self) -> bool {
        self.can_enqueue
    }

    /// The ids of the episodes that are finished on disk.
    pub fn downloaded(&self, entry: &Entry, episodes: &[Item], _info: &SourceInfo) -> HashSet<i64> {
        let states = download_states(self.host.as_ref(), Kind::Anime, entry, episodes);
        states.into_iter().filter(|(_, state)| *state == DownloadState::Downloaded).map(|(id, _)| id).collect()
    }

    /// Queues the episodes and starts the queue. `false` when nothing was queued. `start_now` puts the one episode at
    /// the front and starts the run.
    pub fn enqueue(&self, entry: &Entry, episodes: &[Item], _info: &SourceInfo, start_now: bool) -> bool {
        if !self.can_enqueue {
            return false;
        }
        if start_now {
            let Some(first) = episodes.first() else { return false };
            self.host.start_now(Kind::Anime, first.id);
            return true;
        }
        let ids: Vec<i64> = episodes.iter().map(|e| e.id).collect();
        let queued = self.host.enqueue(Kind::Anime, entry.id, &ids);
        if queued {
            self.host.start_queue(Kind::Anime);
        }
        queued
    }

    /// Deletes the downloads of these episodes: they leave the queue and their folders go. The bookmark rule does not
    /// keep any back (an explicit delete); the entry's categories no longer matter.
    pub fn delete(&self, entry: &Entry, episodes: &[Item], _categories: Vec<i64>, _info: &SourceInfo) {
        let ids: Vec<i64> = episodes.iter().map(|e| e.id).collect();
        self.host.delete(Kind::Anime, entry.id, &ids);
    }
}

#[cfg(test)]
mod tests {
    use kuro_platform::kind::Kind;

    use super::*;

    fn info() -> SourceInfo {
        SourceInfo { name: "Reader (EN)".into(), stub: false }
    }

    fn entry() -> Entry {
        Entry { id: 1, source: 7, title: "Frieren".into(), ..Entry::create(Kind::Anime) }
    }

    fn episodes(n: i64) -> Vec<Item> {
        (1..=n).map(|id| Item { id, entry_id: 1, name: format!("c{id}"), url: format!("/e{id}"), ..Item::create(Kind::Anime) }).collect()
    }

    #[test]
    fn the_page_downloads_go_through_the_one_service() {
        use crate::updates::download_host::fake::{Call, Fake};

        let fake = Arc::new(Fake::installed());
        fake.disk.lock().unwrap().push(2);
        let downloads = EntryDownloads::over(fake.clone(), true);
        assert!(downloads.can_enqueue());
        assert_eq!(downloads.downloaded(&entry(), &episodes(3), &info()), HashSet::from([2]));

        assert!(downloads.enqueue(&entry(), &episodes(2), &info(), false));
        assert!(downloads.enqueue(&entry(), &episodes(2)[1..], &info(), true));
        downloads.delete(&entry(), &episodes(2)[1..], Vec::new(), &info());
        assert_eq!(
            fake.calls(),
            [
                Call::Enqueue { entry_id: 1, unit_ids: vec![1, 2] },
                Call::StartQueue,
                Call::StartNow(2),
                Call::Delete { entry_id: 1, unit_ids: vec![2] },
            ]
        );
    }

    #[test]
    fn without_the_service_nothing_is_queued() {
        let downloads = EntryDownloads::over(Arc::new(crate::updates::download_host::fake::Fake::default()), false);
        assert!(!downloads.can_enqueue());
        assert!(!downloads.enqueue(&entry(), &episodes(1), &info(), false));
    }
}
