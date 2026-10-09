//! Player domain types (area 45 part 2, D§1.1-1.4; T-45b-001). Plain data, no libmpv and no GTK.
//! `Video::video_url == "null"` is the lazy sentinel of the legacy constructor (`Video.kt:69`);
//! `Video::is_lazy` is the only place that reads it.

use serde_json::{Map, Value};

/// The literal `videoUrl` that means "resolve lazily via `getVideoUrl`" (Video.kt:69).
pub const LAZY_URL: &str = "null";

/// `Hoster.NO_HOSTER_LIST` (Hoster.kt:57), the name of the hoster that wraps a bare video list.
pub const NO_HOSTER_LIST: &str = "no_hoster_list";

/// `Video.State` (Video.kt:180-185). `Queue` is the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VideoState {
    #[default]
    Queue,
    LoadVideo,
    Ready,
    Error,
}

/// A side track of a video: `Track(url, lang)` (Video.kt:11).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Track {
    pub url: String,
    pub lang: String,
}

/// `ChapterType` (Video.kt:14-20). `Other` is the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChapterType {
    Opening,
    Ending,
    Recap,
    MixedOp,
    #[default]
    Other,
}

/// `TimeStamp(start, end, name, type)` in seconds (Video.kt:23-28).
#[derive(Clone, Debug, PartialEq)]
pub struct TimeStamp {
    pub start: f64,
    pub end: f64,
    pub name: String,
    pub kind: ChapterType,
}

/// One playable stream (`Video`, D§1.1). Every field keeps the Kotlin default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Video {
    /// Empty until resolved. `"null"` means lazy (see `is_lazy`).
    pub video_url: String,
    pub video_title: String,
    pub resolution: Option<i32>,
    pub bitrate: Option<i32>,
    /// `None` means "use the source headers".
    pub headers: Option<Vec<(String, String)>>,
    pub preferred: bool,
    pub subtitle_tracks: Vec<Track>,
    pub audio_tracks: Vec<Track>,
    pub timestamps: Vec<TimeStamp>,
    pub mpv_args: Vec<(String, String)>,
    pub ffmpeg_stream_args: Vec<(String, String)>,
    pub ffmpeg_video_args: Vec<(String, String)>,
    pub internal_data: String,
    /// Set once `get_resolved_video` has run for this video.
    pub initialized: bool,
    pub memo: Map<String, Value>,
    /// Transient (not serialised): the state while a hoster is played.
    pub status: VideoState,
}

impl Video {
    /// True only for the literal `"null"` URL (E-32). A server that returns `"null"` as a real URL
    /// is treated as lazy once per list build (`parse_video_urls`), which is the documented ceiling.
    pub fn is_lazy(&self) -> bool {
        self.video_url == LAZY_URL
    }
}

/// `Hoster.State` (Hoster.kt), the transient status of a hoster.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HosterStatus {
    #[default]
    Idle,
    Loading,
    Ready,
    Error,
}

/// A source of videos (`Hoster`, D§1.2). `video_list` is `None` until the hoster is loaded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hoster {
    pub hoster_url: String,
    pub hoster_name: String,
    pub video_list: Option<Vec<Video>>,
    pub internal_data: String,
    /// Lazy hosters load only when the user clicks them.
    pub lazy: bool,
    pub memo: Map<String, Value>,
    pub status: HosterStatus,
}

impl Hoster {
    /// `List<Video>.toHosterList()`: one hoster with an empty URL that wraps the list.
    pub fn from_videos(videos: Vec<Video>) -> Self {
        Self {
            hoster_name: NO_HOSTER_LIST.to_owned(),
            video_list: Some(videos),
            ..Self::default()
        }
    }
}

/// UI state of a hoster (`HosterState`, QualitySheet.kt:51-60). `Ready` keeps one state per video.
#[derive(Clone, Debug, PartialEq)]
pub enum HosterState {
    Idle(String),
    Loading(String),
    Error(String),
    Ready(ReadyHoster),
}

/// The payload of `HosterState::Ready`: the videos and their parallel states.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadyHoster {
    pub name: String,
    pub video_list: Vec<Video>,
    pub video_state: Vec<VideoState>,
}

impl HosterState {
    /// `HosterState.Ready.getChangedAt` (QualitySheet.kt:62-72): replaces one slot in both lists.
    /// An index past the end leaves both lists as they are. Only `Ready` has lists, so other states
    /// give `None`.
    pub fn with_video(&self, index: usize, video: Video, state: VideoState) -> Option<Self> {
        let Self::Ready(ready) = self else {
            return None;
        };
        let mut out = ready.clone();
        if let Some(slot) = out.video_list.get_mut(index) {
            *slot = video;
        }
        if let Some(slot) = out.video_state.get_mut(index) {
            *slot = state;
        }
        Some(Self::Ready(out))
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Idle(name) | Self::Loading(name) | Self::Error(name) => name,
            Self::Ready(ready) => &ready.name,
        }
    }
}

/// A chapter-thumbnail tile (`TileInfo`, ThumbnailInfo.kt:24-31). Time is in milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TileInfo {
    pub image_index: i32,
    pub time_ms: i64,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Seek-preview thumbnails (`ThumbnailInfo`, ThumbnailInfo.kt:9-12).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThumbnailInfo {
    pub tile_info: Vec<TileInfo>,
    pub image_tile_urls: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(url: &str) -> Video {
        Video {
            video_url: url.to_owned(),
            ..Video::default()
        }
    }

    #[test]
    fn video_defaults_match_d1_1() {
        let v = Video::default();
        assert_eq!(v.video_url, "");
        assert_eq!(v.video_title, "");
        assert_eq!(v.resolution, None);
        assert_eq!(v.headers, None);
        assert!(!v.preferred);
        assert!(!v.initialized);
        assert_eq!(v.status, VideoState::Queue);
        assert!(v.memo.is_empty());
    }

    #[test]
    fn only_the_literal_null_is_lazy() {
        assert!(video("null").is_lazy());
        assert!(!video("").is_lazy());
        assert!(!video("NULL").is_lazy());
        assert!(!video("nullx").is_lazy());
        assert!(!video("https://example.test/a.m3u8").is_lazy());
    }

    #[test]
    fn hoster_from_videos_wraps_the_list() {
        let h = Hoster::from_videos(vec![video("a")]);
        assert_eq!(h.hoster_name, NO_HOSTER_LIST);
        assert_eq!(h.hoster_url, "");
        assert_eq!(h.video_list.as_ref().map(Vec::len), Some(1));
        assert!(!h.lazy);
        assert_eq!(h.status, HosterStatus::Idle);
    }

    #[test]
    fn with_video_replaces_one_slot_in_both_lists() {
        let ready = HosterState::Ready(ReadyHoster {
            name: "h".to_owned(),
            video_list: vec![video("a"), video("b")],
            video_state: vec![VideoState::Queue, VideoState::Queue],
        });
        let HosterState::Ready(out) = ready.with_video(1, video("c"), VideoState::Error).unwrap()
        else {
            panic!("with_video must keep the Ready variant");
        };
        assert_eq!(out.video_list[0].video_url, "a");
        assert_eq!(out.video_list[1].video_url, "c");
        assert_eq!(out.video_state, vec![VideoState::Queue, VideoState::Error]);
    }

    #[test]
    fn with_video_out_of_range_changes_nothing() {
        let ready = HosterState::Ready(ReadyHoster {
            name: "h".to_owned(),
            video_list: vec![video("a")],
            video_state: vec![VideoState::Ready],
        });
        assert_eq!(
            ready.with_video(5, video("z"), VideoState::Error),
            Some(ready)
        );
    }

    #[test]
    fn with_video_is_none_for_states_without_lists() {
        let idle = HosterState::Idle("i".to_owned());
        assert_eq!(idle.with_video(0, video("a"), VideoState::Ready), None);
    }

    #[test]
    fn hoster_state_names() {
        assert_eq!(HosterState::Idle("x".into()).name(), "x");
        assert_eq!(HosterState::Loading("y".into()).name(), "y");
        assert_eq!(HosterState::Error("z".into()).name(), "z");
        let ready = ReadyHoster {
            name: "r".to_owned(),
            video_list: vec![],
            video_state: vec![],
        };
        assert_eq!(HosterState::Ready(ready).name(), "r");
    }

    #[test]
    fn chapter_type_default_is_other() {
        assert_eq!(ChapterType::default(), ChapterType::Other);
    }
}
