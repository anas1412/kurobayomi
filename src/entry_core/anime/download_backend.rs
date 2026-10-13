//! The anime downloads of the page (T-39b-063, T-39b-013): which episodes are on disk, queueing episodes, and deleting
//! them, all through the app's one download service. Without it nothing can be queued and the page shows no Download
//! menu. `install_queue`, `entry_source`, `entry_ref` and `item_ref` stay for the old anime queue until it goes. No GTK here. The calls block on the disk: run them on the blocking pool.

use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

use kuro_data::entry::Entry;
use kuro_data::item::Item;
use kuro_downloads::anime::layout::AnimeLayout;
use kuro_downloads::anime::manager::AnimeQueue;
use kuro_downloads::anime::model::{AnimeRef, AnimeSource, EpisodeRef};
use kuro_downloads::model::DownloadState;
use kuro_platform::kind::Kind;

use crate::entry_core::detail::SourceInfo;
use crate::entry_core::downloaded::download_states;
use crate::updates::download_host::{self, DownloadBackend};

static QUEUE: OnceLock<AnimeQueue> = OnceLock::new();

/// Gives the pages the app's anime download queue. The first call wins.
pub fn install_queue(queue: AnimeQueue) {
    let _ = QUEUE.set(queue);
}

/// The installed queue, if the app started one.
pub fn installed_queue() -> Option<AnimeQueue> {
    QUEUE.get().cloned()
}

/// Splits a source line `Name (EN)` into its name and lowercase language. A line without a language suffix keeps its
/// whole text as the name and has no language.
pub fn split_source_name(shown: &str) -> (String, String) {
    if let Some(open) = shown.rfind(" (")
        && let Some(lang) = shown[open + 2..].strip_suffix(')')
        && (2..=3).contains(&lang.len())
        && lang.chars().all(|c| c.is_ascii_alphabetic())
    {
        return (shown[..open].to_owned(), lang.to_ascii_lowercase());
    }
    (shown.to_owned(), String::new())
}

/// The downloads source of an entry, named by its source line.
pub(crate) fn entry_source(anime: &Entry, info: &SourceInfo) -> AnimeSource {
    let (name, lang) = split_source_name(&info.name);
    AnimeSource { id: anime.source, name, lang, is_http: !info.stub, unmetered: false }
}

pub(crate) fn entry_ref(anime: &Entry) -> AnimeRef {
    AnimeRef { id: anime.id, source_id: anime.source, url: anime.url.clone(), title: anime.title.clone() }
}

pub(crate) fn item_ref(e: &Item) -> EpisodeRef {
    EpisodeRef {
        id: e.id,
        anime_id: e.entry_id,
        url: e.url.clone(),
        name: e.name.clone(),
        scanlator: e.scanlator.clone(),
        seen: e.done,
        bookmark: e.bookmark,
        fillermark: e.fillermark,
        source_order: e.source_order,
        upload_date: e.date_upload,
        episode_number: e.number,
    }
}

/// The anime downloads of the page, over the one download service (`updates::download_host`), the same path the manga
/// page takes.
pub struct AnimeDownloads {
    host: Arc<dyn DownloadBackend>,
    can_enqueue: bool,
}

impl AnimeDownloads {
    /// Over the app's download service. The folder and the old queue are not read any more; the parameters stay until the
    /// route that passes them moves (phase C5). Call it on the GTK thread: the service is found through the shell.
    pub fn new(_layout: AnimeLayout, _queue: Option<AnimeQueue>) -> Self {
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
    pub fn downloaded(&self, anime: &Entry, episodes: &[Item], _info: &SourceInfo) -> HashSet<i64> {
        let states = download_states(self.host.as_ref(), Kind::Anime, anime, episodes);
        states.into_iter().filter(|(_, state)| *state == DownloadState::Downloaded).map(|(id, _)| id).collect()
    }

    /// Queues the episodes and starts the queue. `false` when nothing was queued. `start_now` puts the one episode at
    /// the front and starts the run.
    pub fn enqueue(&self, anime: &Entry, episodes: &[Item], _info: &SourceInfo, start_now: bool) -> bool {
        if !self.can_enqueue {
            return false;
        }
        if start_now {
            let Some(first) = episodes.first() else { return false };
            self.host.start_now(Kind::Anime, first.id);
            return true;
        }
        let ids: Vec<i64> = episodes.iter().map(|e| e.id).collect();
        let queued = self.host.enqueue(Kind::Anime, anime.id, &ids, false);
        if queued {
            self.host.start_queue(Kind::Anime);
        }
        queued
    }

    /// Deletes the downloads of these episodes: they leave the queue and their folders go. The bookmark rule does not
    /// keep any back (an explicit delete); the entry's categories no longer matter.
    pub fn delete(&self, anime: &Entry, episodes: &[Item], _categories: Vec<i64>, _info: &SourceInfo) {
        let ids: Vec<i64> = episodes.iter().map(|e| e.id).collect();
        self.host.delete(Kind::Anime, anime.id, &ids);
    }
}

#[cfg(test)]
mod tests {
    use kuro_platform::kind::Kind;

    use super::*;

    fn info() -> SourceInfo {
        SourceInfo { name: "Reader (EN)".into(), stub: false }
    }

    fn anime() -> Entry {
        Entry { id: 1, source: 7, title: "Frieren".into(), ..Entry::create(Kind::Anime) }
    }

    fn episodes(n: i64) -> Vec<Item> {
        (1..=n).map(|id| Item { id, entry_id: 1, name: format!("Episode {id}"), url: format!("/e{id}"), ..Item::create(Kind::Anime) }).collect()
    }

    #[test]
    fn a_source_line_splits_into_name_and_language() {
        assert_eq!(split_source_name("Reader (EN)"), ("Reader".to_owned(), "en".to_owned()));
        assert_eq!(split_source_name("Some (odd) name (JA)"), ("Some (odd) name".to_owned(), "ja".to_owned()));
        assert_eq!(split_source_name("Local anime"), ("Local anime".to_owned(), String::new()));
        assert_eq!(split_source_name("Name (long)"), ("Name (long)".to_owned(), String::new()));
    }

    #[test]
    fn the_page_downloads_go_through_the_one_service() {
        use crate::updates::download_host::fake::{Call, Fake};

        let fake = Arc::new(Fake::installed());
        fake.disk.lock().unwrap().push(2);
        let downloads = AnimeDownloads::over(fake.clone(), true);
        assert!(downloads.can_enqueue());
        assert_eq!(downloads.downloaded(&anime(), &episodes(3), &info()), HashSet::from([2]));

        assert!(downloads.enqueue(&anime(), &episodes(2), &info(), false));
        assert!(downloads.enqueue(&anime(), &episodes(2)[1..], &info(), true));
        downloads.delete(&anime(), &episodes(2)[1..], Vec::new(), &info());
        assert_eq!(
            fake.calls(),
            [
                Call::Enqueue { entry_id: 1, unit_ids: vec![1, 2], external: false },
                Call::StartQueue,
                Call::StartNow(2),
                Call::Delete { entry_id: 1, unit_ids: vec![2] },
            ]
        );
    }

    #[test]
    fn without_the_service_nothing_is_queued() {
        let downloads = AnimeDownloads::over(Arc::new(crate::updates::download_host::fake::Fake::default()), false);
        assert!(!downloads.can_enqueue());
        assert!(!downloads.enqueue(&anime(), &episodes(1), &info(), false));
    }
}
