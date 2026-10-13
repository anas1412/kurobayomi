//! The anime downloads of the page over `kuro-downloads` (T-39b-063, T-39b-013): which episodes are on disk, queueing
//! episodes, and deleting them. Queueing and the delete rules go through the app's `AnimeQueue` when one is installed
//! (`install_queue`); without one nothing can be queued and the page shows no Download menu, but a delete still removes
//! the episode folders from the downloads folder. No GTK here. The calls block on the disk: run them on the blocking pool.

use std::collections::HashSet;
use std::sync::OnceLock;

use kuro_data::entry::Entry;
use kuro_data::item::Item;
use kuro_downloads::anime::layout::AnimeLayout;
use kuro_downloads::anime::manager::{AnimeQueue, Command, QueueEntry};
use kuro_downloads::anime::model::{AnimeRef, AnimeSource, EpisodeRef};

use crate::entry_core::detail::SourceInfo;

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

/// The downloads folder and, when the app has one, the queue.
pub struct AnimeDownloads {
    layout: AnimeLayout,
    queue: Option<AnimeQueue>,
}

impl AnimeDownloads {
    pub fn new(layout: AnimeLayout, queue: Option<AnimeQueue>) -> Self {
        Self { layout, queue }
    }

    /// Whether episodes can be queued (`onDownloadEpisode != null`).
    pub fn can_enqueue(&self) -> bool {
        self.queue.is_some()
    }

    /// The ids of the episodes that have a finished folder in the downloads folder.
    pub fn downloaded(&self, anime: &Entry, episodes: &[Item], info: &SourceInfo) -> HashSet<i64> {
        let (source, anime) = (entry_source(anime, info), entry_ref(anime));
        episodes.iter().filter(|e| self.layout.find_episode_dir(&source, &anime, &item_ref(e)).is_some()).map(|e| e.id).collect()
    }

    /// Queues the episodes and starts the run when the queue was empty. `false` when there is no queue. `start_now`
    /// puts the one episode at the front and starts the run.
    pub fn enqueue(&self, anime: &Entry, episodes: &[Item], info: &SourceInfo, start_now: bool) -> bool {
        let Some(queue) = &self.queue else { return false };
        let (source, anime) = (entry_source(anime, info), entry_ref(anime));
        let sent = if start_now {
            let Some(episode) = episodes.first().map(item_ref) else { return false };
            let (reply, _answer) = tokio::sync::oneshot::channel();
            queue.send(Command::StartNow { entry: QueueEntry { source, anime, episode, alt: false, video: None }, reply })
        } else {
            let episodes = episodes.iter().map(item_ref).collect();
            let (reply, _answer) = tokio::sync::oneshot::channel();
            queue.send(Command::Enqueue { source, anime, episodes, auto_start: true, alt: false, video: None, reply })
        };
        sent.is_ok()
    }

    /// Deletes the downloads of these episodes: through the queue, which applies the delete rules and keeps its index,
    /// or straight from the folder when there is none. `categories` are the entry's, for the rules.
    pub fn delete(&self, anime: &Entry, episodes: &[Item], categories: Vec<i64>, info: &SourceInfo) {
        let (source, anime) = (entry_source(anime, info), entry_ref(anime));
        let episodes: Vec<EpisodeRef> = episodes.iter().map(item_ref).collect();
        if let Some(queue) = &self.queue {
            let (reply, _answer) = tokio::sync::oneshot::channel();
            let _ = queue.send(Command::DeleteEpisodes { source, anime, episodes, categories, reply });
            return;
        }
        let (_, finished) = self.layout.find_episode_dirs(&source, &anime, &episodes);
        for path in finished.into_iter().chain(self.layout.find_episode_tmp_dirs(&source, &anime, &episodes)) {
            if let Err(e) = std::fs::remove_dir_all(&path) {
                tracing::warn!("could not delete the download {}: {e}", path.display());
            }
        }
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

    /// A downloads folder with the finished folders of `names` for `Frieren`.
    fn root_with(names: &[&str]) -> (tempfile::TempDir, AnimeLayout) {
        let dir = tempfile::tempdir().unwrap();
        let anime_dir = dir.path().join("Reader (EN)").join("Frieren");
        for name in names {
            std::fs::create_dir_all(anime_dir.join(name)).unwrap();
        }
        let layout = AnimeLayout::new(Some(dir.path().to_path_buf()));
        (dir, layout)
    }

    #[test]
    fn a_source_line_splits_into_name_and_language() {
        assert_eq!(split_source_name("Reader (EN)"), ("Reader".to_owned(), "en".to_owned()));
        assert_eq!(split_source_name("Some (odd) name (JA)"), ("Some (odd) name".to_owned(), "ja".to_owned()));
        assert_eq!(split_source_name("Local anime"), ("Local anime".to_owned(), String::new()));
        assert_eq!(split_source_name("Name (long)"), ("Name (long)".to_owned(), String::new()));
    }

    #[test]
    fn only_the_episodes_with_a_folder_are_downloaded() {
        let (_dir, layout) = root_with(&["Episode 2"]);
        let downloads = AnimeDownloads::new(layout, None);
        let found = downloads.downloaded(&anime(), &episodes(3), &info());
        assert_eq!(found, HashSet::from([2]));
    }

    #[test]
    fn without_a_queue_nothing_is_queued_and_a_delete_removes_the_folders() {
        let (dir, layout) = root_with(&["Episode 1", "Episode 2"]);
        std::fs::create_dir_all(dir.path().join("Reader (EN)").join("Frieren").join("Episode 2_tmp")).unwrap();
        let downloads = AnimeDownloads::new(layout, None);
        assert!(!downloads.can_enqueue());
        assert!(!downloads.enqueue(&anime(), &episodes(1), &info(), false));
        downloads.delete(&anime(), &episodes(2)[1..], Vec::new(), &info());
        let left = downloads.downloaded(&anime(), &episodes(2), &info());
        assert_eq!(left, HashSet::from([1]), "episode 2 is gone and episode 1 stays");
        assert!(!dir.path().join("Reader (EN)").join("Frieren").join("Episode 2_tmp").exists(), "a partial download goes too");
    }
}
