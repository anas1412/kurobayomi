//! `Video` and its parts (`Video.kt`, `Track`, `ChapterType`, `TimeStamp`; T-16a-014, T-16a-015, T-16a-016).
//!
//! Dropped with the legacy constructors (ext-lib 15/16 APKs are not loaded, so nothing produces them): `quality`,
//! `url` and `videoPageUrl`, the deprecated `copy`, and the `"null"` URL sentinel (E-22). Runtime state lives in
//! [`VideoEntry`], not in `Video`, so equality compares persisted fields only (E-34).

use serde_json::{Map, Value};

use crate::error::SourceError;

/// An ordered, duplicate-allowing header list (`okhttp3.Headers`). On a [`Video`], `None` means "use the source
/// headers" and `Some` with no pairs means "send none" (E-23).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Headers(pub Vec<(String, String)>);

/// A side track (audio or subtitles) of a video.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub url: String,
    pub lang: String,
}

/// The kind of a chapter marker. Stored by name; `Other` is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChapterType {
    Opening,
    Ending,
    Recap,
    MixedOp,
    #[default]
    Other,
}

/// A chapter marker in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeStamp {
    pub start: f64,
    pub end: f64,
    pub name: String,
    pub kind: ChapterType,
}

impl TimeStamp {
    /// `None` (with a warning) for a non-finite bound or `end < start` (E-25). The source's entry is dropped.
    pub fn new(start: f64, end: f64, name: impl Into<String>, kind: ChapterType) -> Option<Self> {
        if !start.is_finite() || !end.is_finite() || end < start {
            tracing::warn!(start, end, "dropping a chapter marker with invalid bounds");
            return None;
        }
        Some(Self { start, end, name: name.into(), kind })
    }
}

/// One stream of an episode. Every persisted field is here; `Default` gives the Kotlin defaults.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Video {
    /// Empty until the source resolves it.
    pub video_url: String,
    pub video_title: String,
    pub resolution: Option<i32>,
    pub bitrate: Option<i32>,
    pub headers: Option<Headers>,
    /// Selection tier 1 (behaviour 6.1).
    pub preferred: bool,
    pub subtitle_tracks: Vec<Track>,
    pub audio_tracks: Vec<Track>,
    pub timestamps: Vec<TimeStamp>,
    pub mpv_args: Vec<(String, String)>,
    pub ffmpeg_stream_args: Vec<(String, String)>,
    pub ffmpeg_video_args: Vec<(String, String)>,
    pub internal_data: String,
    /// `true` once resolved. An initialised video is never sent to `resolve_video` again.
    pub initialized: bool,
    pub memo: Map<String, Value>,
}

impl Video {
    pub fn new(video_url: impl Into<String>, video_title: impl Into<String>) -> Self {
        Self { video_url: video_url.into(), video_title: video_title.into(), ..Default::default() }
    }

    /// Whether any URL of this video points at the local stream server (`usesHttpServer`, behaviour 8.4).
    pub fn uses_local_server(&self) -> bool {
        is_placeholder(&self.video_url)
            || self.audio_tracks.iter().chain(&self.subtitle_tracks).any(|t| is_placeholder(&t.url))
    }

    /// Points every placeholder URL at the local server on `port` (`copyHttpServer`, behaviour 8.5). Every other field,
    /// including `memo`, is copied as it is (E-07). Port 0 is an error, never a rewrite to `localhost:0` (E-08).
    pub fn rewrite_to_local(&self, port: u16) -> Result<Video, SourceError> {
        if port == 0 {
            return Err(SourceError::LocalServer("port 0 is not a listening port".into()));
        }
        let mut out = self.clone();
        out.video_url = rewrite_placeholders(&self.video_url, port);
        for track in out.audio_tracks.iter_mut().chain(out.subtitle_tracks.iter_mut()) {
            track.url = rewrite_placeholders(&track.url, port);
        }
        Ok(out)
    }
}

/// The placeholder host the sources use for "answer me from the local server" (`HttpServer.PLACEHOLDER_URL`).
pub const PLACEHOLDER_PREFIX: &str = "http://localhost:1";

/// Whether a URL contains a placeholder: `http://localhost:1` not followed by a digit (`localUrl` regex, behaviour 8.4).
/// A port such as `12345` is not a placeholder. The `regex` crate has no lookahead, so the check is done by hand.
pub fn is_placeholder(url: &str) -> bool {
    placeholder_starts(url).next().is_some()
}

fn placeholder_starts(url: &str) -> impl Iterator<Item = usize> + '_ {
    url.match_indices(PLACEHOLDER_PREFIX)
        .map(|(at, _)| at)
        .filter(move |&at| !url[at + PLACEHOLDER_PREFIX.len()..].starts_with(|c: char| c.is_ascii_digit()))
}

/// Replaces every placeholder with `http://localhost:<port>` and keeps the rest of each URL.
fn rewrite_placeholders(url: &str, port: u16) -> String {
    let mut out = String::with_capacity(url.len());
    let mut last = 0;
    for at in placeholder_starts(url) {
        out.push_str(&url[last..at]);
        out.push_str(&format!("http://localhost:{port}"));
        last = at + PLACEHOLDER_PREFIX.len();
    }
    out.push_str(&url[last..]);
    out
}

/// The state of one video while a hoster is played (`Video.State`, runtime only, not in equality).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoState {
    #[default]
    Queue,
    LoadVideo,
    Ready,
    Error,
}

/// A video with its runtime state (E-34).
#[derive(Debug, Clone)]
pub struct VideoEntry {
    pub video: Video,
    pub state: VideoState,
}

impl VideoEntry {
    /// A video waiting in the queue, the state every video starts in (behaviour 5.2).
    pub fn queued(video: Video) -> Self {
        Self { video, state: VideoState::Queue }
    }
}

/// Equality compares the persisted [`Video`] only. The runtime `state` is not part of it (E-34).
impl PartialEq for VideoEntry {
    fn eq(&self, other: &Self) -> bool {
        self.video == other.video
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_url(url: &str) -> Video {
        Video::new(url, "A")
    }

    #[test]
    fn the_defaults_match_the_kotlin_constructor() {
        let v = Video::default();
        assert!(v.video_url.is_empty() && v.video_title.is_empty() && v.internal_data.is_empty());
        assert_eq!((v.resolution, v.bitrate, v.headers.clone()), (None, None, None));
        assert!(!v.preferred && !v.initialized);
        assert!(v.subtitle_tracks.is_empty() && v.audio_tracks.is_empty() && v.timestamps.is_empty());
        assert!(v.memo.is_empty());
        assert_eq!(VideoEntry::queued(v).state, VideoState::Queue);
    }

    #[test]
    fn equality_compares_persisted_fields_and_ignores_runtime_state() {
        let a = VideoEntry::queued(with_url("https://v/1.m3u8"));
        let b = VideoEntry { state: VideoState::Ready, ..a.clone() };
        assert_ne!(a.state, b.state);
        assert_eq!(a, b);
        assert_eq!(a.video, b.video);
        let mut changed = with_url("https://v/1.m3u8");
        changed.preferred = true;
        assert_ne!(changed, with_url("https://v/1.m3u8"));
    }

    #[test]
    fn headers_none_and_empty_are_distinct() {
        let none = Video::default();
        let empty = Video { headers: Some(Headers(Vec::new())), ..Default::default() };
        assert_ne!(none, empty);
        assert_eq!(none.headers, None);
    }

    #[test]
    fn timestamps_reject_non_finite_bounds_and_reversed_ranges() {
        assert!(TimeStamp::new(f64::NAN, 1.0, "x", ChapterType::Opening).is_none());
        assert!(TimeStamp::new(0.0, f64::INFINITY, "x", ChapterType::Opening).is_none());
        assert!(TimeStamp::new(5.0, 1.0, "x", ChapterType::Opening).is_none());
        let ok = TimeStamp::new(1.0, 1.0, "op", ChapterType::MixedOp).unwrap();
        assert_eq!((ok.start, ok.end, ok.kind), (1.0, 1.0, ChapterType::MixedOp));
        assert_eq!(ChapterType::default(), ChapterType::Other);
    }

    #[test]
    fn placeholder_detection_matches_the_regex_rules() {
        assert!(is_placeholder("http://localhost:1/x"));
        assert!(is_placeholder("http://localhost:1"));
        assert!(!is_placeholder("http://localhost:12/x"));
        assert!(!is_placeholder("https://localhost:1/x"));
        assert!(!is_placeholder("http://localhost:10/"));
    }

    #[test]
    fn uses_local_server_checks_the_video_and_every_track() {
        assert!(with_url("http://localhost:1/s.m3u8").uses_local_server());
        let only_audio = Video {
            audio_tracks: vec![Track { url: "http://localhost:1/a".into(), lang: "en".into() }],
            ..Default::default()
        };
        assert!(only_audio.uses_local_server());
        assert!(!with_url("https://cdn/x.m3u8").uses_local_server());
    }

    #[test]
    fn rewrite_to_local_rewrites_every_placeholder_and_keeps_the_other_fields() {
        let mut original = Video {
            video_url: "http://localhost:1/a?x=1".into(),
            headers: Some(Headers(vec![("k".into(), "v".into())])),
            timestamps: vec![TimeStamp::new(0.0, 1.0, "t", ChapterType::Recap).unwrap()],
            subtitle_tracks: vec![Track { url: "http://localhost:1".into(), lang: "en".into() }],
            ..Default::default()
        };
        original.memo.insert("k".into(), Value::Bool(true));
        let rewritten = original.rewrite_to_local(8123).unwrap();
        assert_eq!(rewritten.video_url, "http://localhost:8123/a?x=1");
        assert_eq!(rewritten.subtitle_tracks[0].url, "http://localhost:8123");
        assert_eq!(rewritten.memo, original.memo);
        assert_eq!(rewritten.headers, original.headers);
        assert_eq!(rewritten.timestamps, original.timestamps);
        assert!(!rewritten.uses_local_server());
    }

    #[test]
    fn a_digit_port_after_the_placeholder_is_left_alone() {
        let rewritten = with_url("http://localhost:12/x").rewrite_to_local(9000).unwrap();
        assert_eq!(rewritten.video_url, "http://localhost:12/x");
    }

    #[test]
    fn rewrite_to_port_zero_is_a_local_server_error() {
        let err = with_url("http://localhost:1/x").rewrite_to_local(0).unwrap_err();
        assert!(matches!(err, SourceError::LocalServer(_)));
    }
}
