//! Anime queue element and the references it carries (port of `AnimeDownload.kt` and the fields of Episode and Anime
//! that the engine reads). Identity is the episode id; no `PartialEq` over the mutable `video`.

use std::sync::Mutex;

use tokio::sync::watch;

use crate::anime::progress::ProgressCell;
use crate::model::DownloadState;

/// The installed source an episode comes from. `is_http` is false for local sources, which the queue ignores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimeSource {
    pub id: i64,
    pub name: String,
    pub lang: String,
    pub is_http: bool,
    /// Maps `UnmeteredSource`: left out of the queue-size warnings.
    pub unmetered: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimeRef {
    pub id: i64,
    pub source_id: i64,
    pub url: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeRef {
    pub id: i64,
    pub anime_id: i64,
    pub url: String,
    pub name: String,
    pub scanlator: Option<String>,
    pub seen: bool,
    pub bookmark: bool,
    pub fillermark: bool,
    pub source_order: i64,
    /// Kotlin `dateUpload`, epoch milliseconds. Used by the queue sort (T-24a-045).
    pub upload_date: i64,
    /// Kotlin `episodeNumber`. Used by the queue sort (T-24a-045).
    pub episode_number: f64,
}

/// A playable stream chosen by the hoster resolver (Kotlin `Video`, source-api `model/Video.kt`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Video {
    /// The main stream. A local file for offline playback, `file://<path>`.
    pub url: String,
    pub title: String,
    /// Per-video headers. `None` falls back to the source headers (behaviour 5.7 step 1).
    pub headers: Option<Vec<(String, String)>>,
    pub subtitle_tracks: Vec<Track>,
    pub audio_tracks: Vec<Track>,
    /// `ffmpeg_stream_args`: `-k v` pairs placed before the main input.
    pub stream_args: Vec<(String, String)>,
    /// `ffmpeg_video_args`: `-k v` pairs placed before the output path.
    pub video_args: Vec<(String, String)>,
    /// Set once the stream is resolved (`Video.initialized`).
    pub initialized: bool,
    pub status: VideoStatus,
}

/// A subtitle or audio track next to the main stream. `lang` is used as the track title.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Track {
    pub url: String,
    pub lang: String,
}

/// Load state of a [`Video`] (`Video.State`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VideoStatus {
    #[default]
    Queue,
    LoadVideo,
    Ready,
    Error,
}

/// One queue item. The manager actor owns the list; the runner reads it and reports status through `state`.
pub struct AnimeDownload {
    pub source: AnimeSource,
    pub anime: AnimeRef,
    pub episode: EpisodeRef,
    /// Selects the alternative downloader (`alt`, behaviour 5.1).
    pub change_downloader: bool,
    video: Mutex<Option<Video>>,
    pub state: watch::Sender<DownloadState>,
    pub progress: ProgressCell,
}

impl AnimeDownload {
    pub fn new(
        source: AnimeSource,
        anime: AnimeRef,
        episode: EpisodeRef,
        change_downloader: bool,
        video: Option<Video>,
    ) -> Self {
        Self {
            source,
            anime,
            episode,
            change_downloader,
            video: Mutex::new(video),
            state: watch::channel(DownloadState::Queue).0,
            progress: ProgressCell::new(),
        }
    }

    pub fn status(&self) -> DownloadState {
        *self.state.borrow()
    }

    /// Sets the status. Subscribers are woken only when the value changes (T-24a-046).
    pub fn set_status(&self, status: DownloadState) {
        self.state.send_if_modified(|current| {
            if *current == status {
                false
            } else {
                *current = status;
                true
            }
        });
    }

    /// A receiver for this item's status. Its first value is the current status, and `changed()` wakes only on a later
    /// change (T-24a-046).
    pub fn subscribe_status(&self) -> watch::Receiver<DownloadState> {
        self.state.subscribe()
    }

    pub fn video(&self) -> Option<Video> {
        self.video.lock().ok().and_then(|v| v.clone())
    }

    /// Stores the resolved video. Resolution happens only while `video` is `None` (behaviour 5.2).
    pub fn set_video(&self, video: Video) {
        if let Ok(mut slot) = self.video.lock() {
            *slot = Some(video);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn item(id: i64) -> Arc<AnimeDownload> {
        let source = AnimeSource { id: 1, name: "S".into(), lang: "en".into(), is_http: true, unmetered: false };
        let anime = AnimeRef { id: 10, source_id: 1, url: "/a".into(), title: "A".into() };
        let episode = EpisodeRef {
            id,
            anime_id: 10,
            url: "/e".into(),
            name: "E".into(),
            scanlator: None,
            seen: false,
            bookmark: false,
            fillermark: false,
            source_order: id,
            upload_date: 0,
            episode_number: 0.0,
        };
        Arc::new(AnimeDownload::new(source, anime, episode, false, None))
    }

    #[test]
    fn lookups_by_episode_id_survive_a_later_video_change() {
        let mut map: HashMap<i64, Arc<AnimeDownload>> = HashMap::new();
        let download = item(7);
        map.insert(7, download.clone());
        download.set_video(Video { url: "https://x/v.mkv".into(), ..Default::default() });
        assert!(map.get(&7).is_some_and(|d| d.video().is_some()), "the entry is still found after the video changed");
    }
}
