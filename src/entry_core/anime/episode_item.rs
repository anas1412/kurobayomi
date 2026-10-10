//! The episode list item (T-39b-002): the episode with its download state, its download progress and its selected flag.
//! Upstream: `AnimeScreenModel.kt:626-653` (`toEpisodeListItems`) and `AnimeScreenModel.kt:595-606` (`updateDownloadState`).

use kuro_data::episode::Episode;
use kuro_downloads::DownloadState;

#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeItem {
    pub episode: Episode,
    pub download: DownloadState,
    /// The queued download's progress (0 to 100); 0 when there is no queued download.
    pub progress: i32,
    pub selected: bool,
}

/// The download state and progress of one episode. A local entry counts every episode as downloaded and runs no lookup
/// (`queued` and `on_disk` are not called). Otherwise a queued download wins, with its own state and progress, over an
/// on-disk copy (T-DL-6). Without either the episode is not downloaded.
pub fn derive_download(
    is_local: bool,
    queued: impl FnOnce() -> Option<(DownloadState, i32)>,
    on_disk: impl FnOnce() -> bool,
) -> (DownloadState, i32) {
    if is_local {
        return (DownloadState::Downloaded, 0);
    }
    if let Some(active) = queued() {
        return active;
    }
    if on_disk() {
        (DownloadState::Downloaded, 0)
    } else {
        (DownloadState::NotDownloaded, 0)
    }
}

impl EpisodeItem {
    /// Builds one item from its episode, the download lookups and the selected flag.
    pub fn new(
        episode: Episode,
        is_local: bool,
        queued: impl FnOnce() -> Option<(DownloadState, i32)>,
        on_disk: impl FnOnce() -> bool,
        selected: bool,
    ) -> Self {
        let (download, progress) = derive_download(is_local, queued, on_disk);
        Self { episode, download, progress, selected }
    }

    /// True only for `Downloaded`: a queued, running or failed episode is not downloaded.
    pub fn is_downloaded(&self) -> bool {
        self.download == DownloadState::Downloaded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn episode(id: i64) -> Episode {
        let mut e = Episode::create();
        e.id = id;
        e
    }

    #[test]
    fn a_queued_download_beats_the_copy_on_disk() {
        let item = EpisodeItem::new(episode(1), false, || Some((DownloadState::Downloading, 42)), || true, false);
        assert_eq!(item.download, DownloadState::Downloading);
        assert_eq!(item.progress, 42);
        assert!(!item.is_downloaded());
    }

    #[test]
    fn a_failed_queued_download_still_beats_the_copy_on_disk() {
        let item = EpisodeItem::new(episode(1), false, || Some((DownloadState::Error, 0)), || true, false);
        assert_eq!(item.download, DownloadState::Error);
    }

    #[test]
    fn an_episode_on_disk_with_no_queue_entry_is_downloaded_with_no_progress() {
        let item = EpisodeItem::new(episode(1), false, || None, || true, false);
        assert_eq!((item.download, item.progress), (DownloadState::Downloaded, 0));
        assert!(item.is_downloaded());
    }

    #[test]
    fn an_episode_nowhere_is_not_downloaded() {
        let item = EpisodeItem::new(episode(1), false, || None, || false, false);
        assert_eq!(item.download, DownloadState::NotDownloaded);
        assert!(!item.is_downloaded());
    }

    #[test]
    fn a_local_entry_is_downloaded_and_runs_no_lookup() {
        let item = EpisodeItem::new(
            episode(1),
            true,
            || panic!("the queue is not read for a local entry"),
            || panic!("the disk is not read for a local entry"),
            true,
        );
        assert_eq!(item.download, DownloadState::Downloaded);
        assert!(item.is_downloaded());
        assert!(item.selected);
    }

    #[test]
    fn is_downloaded_is_true_only_for_downloaded() {
        for state in [DownloadState::NotDownloaded, DownloadState::Queue, DownloadState::Downloading, DownloadState::Error] {
            let item = EpisodeItem { episode: episode(1), download: state, progress: 0, selected: false };
            assert!(!item.is_downloaded(), "{state:?}");
        }
    }
}
